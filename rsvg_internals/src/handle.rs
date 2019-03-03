use std::cell::{Cell, RefCell};
use std::ffi::CString;
use std::ptr;
use std::rc::Rc;

use cairo::{self, ImageSurface, Status};
use gdk_pixbuf::Pixbuf;
use gio::{self, FileExt};
use glib::{self, Bytes, Cast};
use glib_sys;
use libc;
use locale_config::{LanguageRange, Locale};

use allowed_url::{AllowedUrl, Href};
use c_api::{RsvgDimensionData, RsvgPositionData, RsvgSizeFunc};
use dpi::Dpi;
use drawing_ctx::{DrawingCtx, RsvgRectangle};
use error::{DefsLookupErrorKind, LoadingError, RenderingError};
use node::RsvgNode;
use pixbuf_utils::{empty_pixbuf, pixbuf_from_surface};
use structure::{IntrinsicDimensions, NodeSvg};
use surface_utils::{shared_surface::SharedImageSurface, shared_surface::SurfaceType};
use svg::Svg;
use url::Url;
use util::rsvg_g_warning;
use xml::XmlState;
use xml2_load::xml_state_load_from_possibly_compressed_stream;

/// Flags used during loading
///
/// We communicate these to/from the C code with a HandleFlags
/// and this struct provides to_flags() and from_flags() methods.
#[derive(Default, Copy, Clone)]
pub struct LoadFlags {
    /// Whether to turn off size limits in libxml2
    pub unlimited_size: bool,

    /// Whether to keep original (undecoded) image data to embed in Cairo PDF surfaces
    pub keep_image_data: bool,
}

#[derive(Clone)]
pub struct LoadOptions {
    pub flags: LoadFlags,
    pub base_url: Option<Url>,
    pub locale: Locale,
}

impl LoadOptions {
    fn new(flags: LoadFlags, base_url: Option<Url>, locale: Locale) -> LoadOptions {
        LoadOptions {
            flags,
            base_url,
            locale,
        }
    }

    pub fn copy_with_base_url(&self, base_url: &AllowedUrl) -> LoadOptions {
        LoadOptions {
            flags: self.flags,
            base_url: Some((*base_url).clone()),
            locale: self.locale.clone(),
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum LoadState {
    Start,
    Loading,
    ClosedOk,
    ClosedError,
}

struct SizeCallback {
    size_func: RsvgSizeFunc,
    user_data: glib_sys::gpointer,
    destroy_notify: glib_sys::GDestroyNotify,
}

impl SizeCallback {
    fn call(&self, width: libc::c_int, height: libc::c_int) -> (libc::c_int, libc::c_int) {
        unsafe {
            let mut w = width;
            let mut h = height;

            if let Some(ref f) = self.size_func {
                f(&mut w, &mut h, self.user_data);
            };

            (w, h)
        }
    }
}

impl Default for SizeCallback {
    fn default() -> SizeCallback {
        SizeCallback {
            size_func: None,
            user_data: ptr::null_mut(),
            destroy_notify: None,
        }
    }
}

impl Drop for SizeCallback {
    fn drop(&mut self) {
        unsafe {
            if let Some(ref f) = self.destroy_notify {
                f(self.user_data);
            };
        }
    }
}

pub struct Handle {
    pub dpi: Cell<Dpi>,
    pub base_url: RefCell<Option<Url>>,
    base_url_cstring: RefCell<Option<CString>>, // needed because the C api returns *const char
    svg: RefCell<Option<Rc<Svg>>>,
    pub load_flags: Cell<LoadFlags>,
    load_state: Cell<LoadState>,
    buffer: RefCell<Vec<u8>>, // used by the legacy write() api
    size_callback: RefCell<SizeCallback>,
    in_loop: Cell<bool>,
    is_testing: Cell<bool>,
}

impl Handle {
    pub fn new() -> Handle {
        Handle {
            dpi: Cell::new(Dpi::default()),
            base_url: RefCell::new(None),
            base_url_cstring: RefCell::new(None),
            svg: RefCell::new(None),
            load_flags: Cell::new(LoadFlags::default()),
            load_state: Cell::new(LoadState::Start),
            buffer: RefCell::new(Vec::new()),
            size_callback: RefCell::new(SizeCallback::default()),
            in_loop: Cell::new(false),
            is_testing: Cell::new(false),
        }
    }

    pub fn new_with_flags(load_flags: LoadFlags) -> Handle {
        let handle = Handle::new();
        handle.load_flags.set(load_flags);
        handle
    }

    // from the public API
    pub fn set_base_url(&self, url: &str) {
        if self.load_state.get() != LoadState::Start {
            panic!("Please set the base file or URI before loading any data into RsvgHandle",);
        }

        match Url::parse(&url) {
            Ok(u) => {
                let url_cstring = CString::new(u.as_str()).unwrap();

                rsvg_log!("setting base_uri to \"{}\"", u.as_str());
                *self.base_url.borrow_mut() = Some(u);
                *self.base_url_cstring.borrow_mut() = Some(url_cstring);
            }

            Err(e) => {
                rsvg_log!(
                    "not setting base_uri to \"{}\" since it is invalid: {}",
                    url,
                    e
                );
            }
        }
    }

    pub fn set_base_gfile(&self, file: &gio::File) {
        if let Some(uri) = file.get_uri() {
            self.set_base_url(&uri);
        } else {
            panic!("file has no URI; will not set the base URI");
        }
    }

    pub fn get_base_url_as_ptr(&self) -> *const libc::c_char {
        match *self.base_url_cstring.borrow() {
            None => ptr::null(),
            Some(ref url) => url.as_ptr(),
        }
    }

    pub fn set_size_callback(
        &self,
        size_func: RsvgSizeFunc,
        user_data: glib_sys::gpointer,
        destroy_notify: glib_sys::GDestroyNotify,
    ) {
        *self.size_callback.borrow_mut() = SizeCallback {
            size_func,
            user_data,
            destroy_notify,
        };
    }

    pub fn read_stream_sync(
        &self,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<(), LoadingError> {
        self.load_state.set(LoadState::Loading);

        let svg =
            Svg::load_from_stream(&self.load_options(), stream, cancellable).map_err(|e| {
                self.load_state.set(LoadState::ClosedError);
                e
            })?;

        *self.svg.borrow_mut() = Some(Rc::new(svg));
        self.load_state.set(LoadState::ClosedOk);
        Ok(())
    }

    pub fn check_is_loaded(self: &Handle) -> Result<(), RenderingError> {
        match self.load_state.get() {
            LoadState::Start => {
                rsvg_g_warning("RsvgHandle has not been loaded");
                Err(RenderingError::HandleIsNotLoaded)
            }

            LoadState::Loading => {
                rsvg_g_warning("RsvgHandle is still loading; call rsvg_handle_close() first");
                Err(RenderingError::HandleIsNotLoaded)
            }

            LoadState::ClosedOk => Ok(()),

            LoadState::ClosedError => {
                rsvg_g_warning(
                    "RsvgHandle could not read or parse the SVG; did you check for errors during \
                     the loading stage?",
                );
                Err(RenderingError::HandleIsNotLoaded)
            }
        }
    }

    pub fn load_state(&self) -> LoadState {
        self.load_state.get()
    }

    fn load_options(&self) -> LoadOptions {
        LoadOptions::new(
            self.load_flags.get(),
            self.base_url.borrow().clone(),
            locale_from_environment(),
        )
    }

    pub fn write(&self, buf: &[u8]) {
        match self.load_state.get() {
            LoadState::Start => self.load_state.set(LoadState::Loading),
            LoadState::Loading => (),
            _ => unreachable!(),
        };

        self.buffer.borrow_mut().extend_from_slice(buf);
    }

    pub fn close(&self) -> Result<(), LoadingError> {
        let res = match self.load_state.get() {
            LoadState::Start => {
                self.load_state.set(LoadState::ClosedError);
                Err(LoadingError::NoDataPassedToParser)
            }

            LoadState::Loading => {
                let buffer = self.buffer.borrow();
                let bytes = Bytes::from(&*buffer);
                let stream = gio::MemoryInputStream::new_from_bytes(&bytes);
                let mut xml = XmlState::new(&self.load_options());

                xml_state_load_from_possibly_compressed_stream(
                    &mut xml,
                    self.load_flags.get(),
                    &stream.upcast(),
                    None,
                )
                .map_err(|e| {
                    self.load_state.set(LoadState::ClosedError);
                    e
                })?;

                let svg = xml.steal_result().map_err(|e| {
                    self.load_state.set(LoadState::ClosedError);
                    e
                })?;

                self.load_state.set(LoadState::ClosedOk);
                *self.svg.borrow_mut() = Some(Rc::new(svg));
                Ok(())
            }

            LoadState::ClosedOk | LoadState::ClosedError => {
                // closing is idempotent
                Ok(())
            }
        };

        assert!(
            self.load_state.get() == LoadState::ClosedOk
                || self.load_state.get() == LoadState::ClosedError
        );

        res
    }

    fn create_drawing_ctx_for_node(
        &self,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
        node: Option<&RsvgNode>,
    ) -> DrawingCtx {
        let mut draw_ctx = DrawingCtx::new(
            self.svg.borrow().as_ref().unwrap().clone(),
            cr,
            viewport,
            self.dpi.get(),
            self.is_testing.get(),
        );

        if let Some(node) = node {
            draw_ctx.add_node_and_ancestors_to_stack(node);
        }

        draw_ctx
    }

    pub fn has_sub(&self, id: &str) -> Result<bool, RenderingError> {
        self.check_is_loaded()?;

        match self.lookup_node(id) {
            Ok(_) => Ok(true),

            Err(DefsLookupErrorKind::NotFound) => Ok(false),

            Err(e) => Err(RenderingError::InvalidId(e)),
        }
    }

    pub fn get_dimensions(&self) -> Result<RsvgDimensionData, RenderingError> {
        self.check_is_loaded()?;

        // This function is probably called from the cairo_render functions,
        // or is being erroneously called within the size_func.
        // To prevent an infinite loop we are saving the state, and
        // returning a meaningless size.
        if self.in_loop.get() {
            return Ok(RsvgDimensionData {
                width: 1,
                height: 1,
                em: 1.0,
                ex: 1.0,
            });
        }

        self.in_loop.set(true);

        let res = self.get_dimensions_sub(None);

        self.in_loop.set(false);

        res
    }

    pub fn get_dimensions_no_error(&self) -> RsvgDimensionData {
        match self.get_dimensions() {
            Ok(dimensions) => dimensions,

            Err(_) => {
                RsvgDimensionData {
                    width: 0,
                    height: 0,
                    em: 0.0,
                    ex: 0.0,
                }

                // This old API doesn't even let us return an error, sigh.
            }
        }
    }

    pub fn get_dimensions_sub(
        &self,
        id: Option<&str>,
    ) -> Result<RsvgDimensionData, RenderingError> {
        self.check_is_loaded()?;

        let (ink_r, _) = self.get_geometry_sub(id)?;

        let (w, h) = self
            .size_callback
            .borrow()
            .call(ink_r.width as libc::c_int, ink_r.height as libc::c_int);

        Ok(RsvgDimensionData {
            width: w,
            height: h,
            em: ink_r.width,
            ex: ink_r.height,
        })
    }

    pub fn get_position_sub(&self, id: Option<&str>) -> Result<RsvgPositionData, RenderingError> {
        self.check_is_loaded()?;

        if let None = id {
            return Ok(RsvgPositionData { x: 0, y: 0 });
        }

        let (ink_r, _) = self.get_geometry_sub(id)?;

        let width = ink_r.width as libc::c_int;
        let height = ink_r.height as libc::c_int;

        self.size_callback.borrow().call(width, height);

        Ok(RsvgPositionData {
            x: ink_r.x as libc::c_int,
            y: ink_r.y as libc::c_int,
        })
    }

    fn get_root(&self) -> RsvgNode {
        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();
        svg.root()
    }

    /// Returns (ink_rect, logical_rect)
    fn get_node_geometry(
        &self,
        node: &RsvgNode,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        // This is just to start with an unknown viewport size
        let dimensions = RsvgDimensionData {
            width: 1,
            height: 1,
            em: 1.0,
            ex: 1.0,
        };

        let target = ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);
        let mut draw_ctx = self.create_drawing_ctx_for_node(
            &cr,
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: f64::from(dimensions.width),
                height: f64::from(dimensions.height),
            },
            Some(node),
        );
        let root = self.get_root();

        draw_ctx.draw_node_from_stack(&root.get_cascaded_values(), &root, false)?;

        let bbox = draw_ctx.get_bbox();

        let ink_rect = bbox
            .ink_rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();
        let logical_rect = bbox
            .rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();

        Ok((ink_rect, logical_rect))
    }

    /// Returns (ink_rect, logical_rect)
    pub fn get_geometry_sub(
        &self,
        id: Option<&str>,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;

        let root = self.get_root();
        let is_root = Rc::ptr_eq(&node, &root);

        if is_root {
            let cascaded = node.get_cascaded_values();
            let values = cascaded.get();

            if let Some((root_width, root_height)) =
                node.with_impl(|svg: &NodeSvg| svg.get_size(&values, self.dpi.get()))
            {
                let ink_r = RsvgRectangle {
                    x: 0.0,
                    y: 0.0,
                    width: f64::from(root_width),
                    height: f64::from(root_height),
                };

                let logical_r = ink_r;

                return Ok((ink_r, logical_r));
            }
        }

        self.get_node_geometry(&node)
    }

    fn get_node_or_root(&self, id: Option<&str>) -> Result<RsvgNode, RenderingError> {
        if let Some(id) = id {
            self.lookup_node(id).map_err(RenderingError::InvalidId)
        } else {
            Ok(self.get_root())
        }
    }

    pub fn get_geometry_for_element(
        &self,
        id: Option<&str>,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;
        self.get_node_geometry(&node)
    }

    fn lookup_node(&self, id: &str) -> Result<RsvgNode, DefsLookupErrorKind> {
        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();

        match Href::parse(&id).map_err(DefsLookupErrorKind::HrefError)? {
            Href::PlainUrl(_) => Err(DefsLookupErrorKind::CannotLookupExternalReferences),
            Href::WithFragment(fragment) => {
                if let Some(uri) = fragment.uri() {
                    // The public APIs to get geometries of individual elements, or to render
                    // them, should only allow referencing elements within the main handle's
                    // SVG file; that is, only plain "#foo" fragment IDs are allowed here.
                    // Otherwise, a calling program could request "another-file#foo" and cause
                    // another-file to be loaded, even if it is not part of the set of
                    // resources that the main SVG actually references.  In the future we may
                    // relax this requirement to allow lookups within that set, but not to
                    // other random files.

                    let msg = format!(
                        "the public API is not allowed to look up external references: {}#{}",
                        uri,
                        fragment.fragment()
                    );

                    rsvg_log!("{}", msg);

                    rsvg_g_warning(&msg);
                    return Err(DefsLookupErrorKind::CannotLookupExternalReferences);
                }

                match svg.lookup_node_by_id(fragment.fragment()) {
                    Some(n) => Ok(n),
                    None => Err(DefsLookupErrorKind::NotFound),
                }
            }
        }
    }

    pub fn render_cairo_sub(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;
        self.check_is_loaded()?;

        let node = if let Some(id) = id {
            Some(self.lookup_node(id).map_err(RenderingError::InvalidId)?)
        } else {
            None
        };

        let dimensions = self.get_dimensions()?;
        let root = self.get_root();

        if dimensions.width == 0 || dimensions.height == 0 {
            // nothing to render
            return Ok(());
        }

        cr.save();
        let mut draw_ctx = self.create_drawing_ctx_for_node(
            cr,
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: f64::from(dimensions.width),
                height: f64::from(dimensions.height),
            },
            node.as_ref(),
        );
        let res = draw_ctx.draw_node_from_stack(&root.get_cascaded_values(), &root, false);
        cr.restore();

        res
    }

    pub fn render_element_to_viewport(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let node = if let Some(id) = id {
            Some(self.lookup_node(id).map_err(RenderingError::InvalidId)?)
        } else {
            None
        };

        let root = self.get_root();

        cr.save();
        let mut draw_ctx = self.create_drawing_ctx_for_node(cr, viewport, node.as_ref());
        let res = draw_ctx.draw_node_from_stack(&root.get_cascaded_values(), &root, false);
        cr.restore();

        res
    }

    pub fn get_pixbuf_sub(&self, id: Option<&str>) -> Result<Pixbuf, RenderingError> {
        self.check_is_loaded()?;

        let dimensions = self.get_dimensions()?;

        if dimensions.width == 0 || dimensions.height == 0 {
            return empty_pixbuf();
        }

        let surface =
            ImageSurface::create(cairo::Format::ARgb32, dimensions.width, dimensions.height)?;

        {
            let cr = cairo::Context::new(&surface);
            self.render_cairo_sub(&cr, id)?;
        }

        let surface = SharedImageSurface::new(surface, SurfaceType::SRgb)?;

        pixbuf_from_surface(&surface)
    }

    pub fn construct_new_from_gfile_sync(
        &self,
        file: &gio::File,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<(), LoadingError> {
        let stream = file.read(cancellable)?;
        self.construct_read_stream_sync(&stream.upcast(), Some(file), cancellable)
    }

    pub fn construct_read_stream_sync(
        &self,
        stream: &gio::InputStream,
        base_file: Option<&gio::File>,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<(), LoadingError> {
        if let Some(file) = base_file {
            self.set_base_gfile(file);
        }

        self.read_stream_sync(stream, cancellable)
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();

        svg.get_intrinsic_dimensions()
    }

    // from the public API
    pub fn set_dpi_x(&self, dpi_x: f64) {
        self.dpi.set(Dpi::new(dpi_x, self.dpi.get().y()));
    }

    // from the public API
    pub fn set_dpi_y(&self, dpi_y: f64) {
        self.dpi.set(Dpi::new(self.dpi.get().x(), dpi_y));
    }

    pub fn set_testing(&self, testing: bool) {
        self.is_testing.set(testing);
    }
}

fn check_cairo_context(cr: &cairo::Context) -> Result<(), RenderingError> {
    let status = cr.status();
    if status == Status::Success {
        Ok(())
    } else {
        let msg = format!(
            "cannot render on a cairo_t with a failure status (status={:?})",
            status,
        );

        rsvg_g_warning(&msg);
        Err(RenderingError::Cairo(status))
    }
}

/// Gets the user's preferred locale from the environment and
/// translates it to a `Locale` with `LanguageRange` fallbacks.
///
/// The `Locale::current()` call only contemplates a single language,
/// but glib is smarter, and `g_get_langauge_names()` can provide
/// fallbacks, for example, when LC_MESSAGES="en_US.UTF-8:de" (USA
/// English and German).  This function converts the output of
/// `g_get_language_names()` into a `Locale` with appropriate
/// fallbacks.
fn locale_from_environment() -> Locale {
    let mut locale = Locale::invariant();

    for name in glib::get_language_names() {
        if let Ok(range) = LanguageRange::from_unix(&name) {
            locale.add(&range);
        }
    }

    locale
}
