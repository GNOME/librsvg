use std;
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
use std::mem;
use std::path::PathBuf;
use std::ptr;
use std::rc::Rc;
use std::slice;

use cairo::{self, ImageSurface, Status};
use cairo_sys;
use gdk_pixbuf::Pixbuf;
use gdk_pixbuf_sys;
use gio::{self, FileExt};
use gio_sys;
use glib::subclass::types::ObjectSubclass;
use glib::translate::*;
use glib::{self, Bytes, Cast};
use glib_sys;
use gobject_sys;
use libc;
use locale_config::{LanguageRange, Locale};

use allowed_url::{AllowedUrl, Href};
use c_api::{get_rust_handle, HandleFlags, RsvgHandle, RsvgHandleFlags};
use dpi::Dpi;
use drawing_ctx::{DrawingCtx, RsvgRectangle};
use error::{set_gerror, DefsLookupErrorKind, LoadingError, RenderingError};
use length::RsvgLength;
use node::RsvgNode;
use pixbuf_utils::{empty_pixbuf, pixbuf_from_surface};
use structure::{IntrinsicDimensions, NodeSvg};
use surface_utils::{shared_surface::SharedImageSurface, shared_surface::SurfaceType};
use svg::Svg;
use url::Url;
use util::rsvg_g_warning;
use xml::XmlState;
use xml2_load::xml_state_load_from_possibly_compressed_stream;

// A *const RsvgHandle is just an opaque pointer we get from C
// #[repr(C)]
// pub struct RsvgHandle {
//    _private: [u8; 0],
// }

// Keep in sync with rsvg.h:RsvgDimensionData
#[repr(C)]
pub struct RsvgDimensionData {
    pub width: libc::c_int,
    pub height: libc::c_int,
    pub em: f64,
    pub ex: f64,
}

// Keep in sync with rsvg.h:RsvgPositionData
#[repr(C)]
pub struct RsvgPositionData {
    pub x: libc::c_int,
    pub y: libc::c_int,
}

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

// Keep in sync with rsvg.h:RsvgSizeFunc
type RsvgSizeFunc = Option<
    unsafe extern "C" fn(
        inout_width: *mut libc::c_int,
        inout_height: *mut libc::c_int,
        user_data: glib_sys::gpointer,
    ),
>;

struct SizeCallback {
    size_func: RsvgSizeFunc,
    user_data: glib_sys::gpointer,
    destroy_notify: glib_sys::GDestroyNotify,
}

impl SizeCallback {
    fn new() -> SizeCallback {
        SizeCallback {
            size_func: None,
            user_data: ptr::null_mut(),
            destroy_notify: None,
        }
    }

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
            size_callback: RefCell::new(SizeCallback::new()),
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

    fn set_base_gfile(&self, file: &gio::File) {
        if let Some(uri) = file.get_uri() {
            self.set_base_url(&uri);
        } else {
            panic!("file has no URI; will not set the base URI");
        }
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

    fn check_is_loaded(self: &Handle) -> Result<(), RenderingError> {
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

    fn get_dimensions_sub(&self, id: Option<&str>) -> Result<RsvgDimensionData, RenderingError> {
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

    fn get_position_sub(&self, id: Option<&str>) -> Result<RsvgPositionData, RenderingError> {
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
    fn get_geometry_sub(
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

    fn get_pixbuf_sub(&self, id: Option<&str>) -> Result<Pixbuf, RenderingError> {
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

    fn construct_new_from_gfile_sync(
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
    pub fn set_load_flags(&self, flags: HandleFlags) {
        self.load_flags.set(LoadFlags::from_flags(flags));
    }

    // from the public API
    pub fn set_dpi_x(&self, dpi_x: f64) {
        self.dpi.set(Dpi::new(dpi_x, self.dpi.get().y()));
    }

    // from the public API
    pub fn set_dpi_y(&self, dpi_y: f64) {
        self.dpi.set(Dpi::new(self.dpi.get().x(), dpi_y));
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

impl LoadFlags {
    pub fn from_flags(flags: HandleFlags) -> Self {
        LoadFlags {
            unlimited_size: flags.contains(HandleFlags::UNLIMITED),
            keep_image_data: flags.contains(HandleFlags::KEEP_IMAGE_DATA),
        }
    }

    pub fn to_flags(&self) -> HandleFlags {
        let mut flags = HandleFlags::empty();

        if self.unlimited_size {
            flags.insert(HandleFlags::UNLIMITED);
        }

        if self.keep_image_data {
            flags.insert(HandleFlags::KEEP_IMAGE_DATA);
        }

        flags
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

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_base_url(
    raw_handle: *const RsvgHandle,
    uri: *const libc::c_char,
) {
    let rhandle = get_rust_handle(raw_handle);

    assert!(!uri.is_null());
    let uri: String = from_glib_none(uri);

    rhandle.set_base_url(&uri);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_base_gfile(
    raw_handle: *const Handle,
) -> *mut gio_sys::GFile {
    let handle = &*raw_handle;

    match *handle.base_url.borrow() {
        None => ptr::null_mut(),
        Some(ref url) => gio::File::new_for_uri(url.as_str()).to_glib_full(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_base_gfile(
    raw_handle: *const RsvgHandle,
    raw_gfile: *mut gio_sys::GFile,
) {
    let rhandle = get_rust_handle(raw_handle);

    assert!(!raw_gfile.is_null());

    let file: gio::File = from_glib_none(raw_gfile);

    rhandle.set_base_gfile(&file);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_base_url(
    raw_handle: *const RsvgHandle,
) -> *const libc::c_char {
    let rhandle = get_rust_handle(raw_handle);

    match *rhandle.base_url_cstring.borrow() {
        None => ptr::null(),
        Some(ref url) => url.as_ptr(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_dpi_x(raw_handle: *const RsvgHandle, dpi_x: f64) {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.dpi.set(Dpi::new(dpi_x, rhandle.dpi.get().y()));
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dpi_x(raw_handle: *const RsvgHandle) -> f64 {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.dpi.get().x()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_dpi_y(raw_handle: *const RsvgHandle, dpi_y: f64) {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.dpi.set(Dpi::new(rhandle.dpi.get().x(), dpi_y));
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dpi_y(raw_handle: *const RsvgHandle) -> f64 {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.dpi.get().y()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_flags(
    raw_handle: *const RsvgHandle,
) -> RsvgHandleFlags {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.load_flags.get().to_flags().to_glib()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_flags(
    raw_handle: *const RsvgHandle,
    flags: RsvgHandleFlags,
) {
    let rhandle = get_rust_handle(raw_handle);

    rhandle
        .load_flags
        .set(LoadFlags::from_flags(from_glib(flags)));
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_size_callback(
    raw_handle: *const RsvgHandle,
    size_func: RsvgSizeFunc,
    user_data: glib_sys::gpointer,
    destroy_notify: glib_sys::GDestroyNotify,
) {
    let rhandle = get_rust_handle(raw_handle);

    *rhandle.size_callback.borrow_mut() = SizeCallback {
        size_func,
        user_data,
        destroy_notify,
    };
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_testing(
    raw_handle: *const RsvgHandle,
    testing: glib_sys::gboolean,
) {
    let rhandle = get_rust_handle(raw_handle);

    rhandle.is_testing.set(from_glib(testing));
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_read_stream_sync(
    handle: *const RsvgHandle,
    stream: *mut gio_sys::GInputStream,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if rhandle.load_state.get() != LoadState::Start {
        panic!("handle must not be already loaded in order to call rsvg_handle_read_stream_sync()",);
    }

    let stream = from_glib_none(stream);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);

    match rhandle.read_stream_sync(&stream, cancellable.as_ref()) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_write(
    handle: *const RsvgHandle,
    buf: *const u8,
    count: usize,
) {
    let rhandle = get_rust_handle(handle);

    let load_state = rhandle.load_state.get();

    if !(load_state == LoadState::Start || load_state == LoadState::Loading) {
        panic!("handle must not be closed in order to write to it");
    }

    let buffer = slice::from_raw_parts(buf, count);

    rhandle.write(buffer);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_close(
    handle: *const RsvgHandle,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    match rhandle.close() {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_geometry_sub(
    handle: *const RsvgHandle,
    out_ink_rect: *mut RsvgRectangle,
    out_logical_rect: *mut RsvgRectangle,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_geometry_sub(id.as_ref().map(String::as_str)) {
        Ok((ink_r, logical_r)) => {
            if !out_ink_rect.is_null() {
                *out_ink_rect = ink_r;
            }

            if !out_logical_rect.is_null() {
                *out_logical_rect = logical_r;
            }

            true.to_glib()
        }

        Err(_) => {
            if !out_ink_rect.is_null() {
                *out_ink_rect = mem::zeroed();
            }

            if !out_logical_rect.is_null() {
                *out_logical_rect = mem::zeroed();
            }

            // FIXME: return a proper error code to the public API
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_has_sub(
    handle: *const RsvgHandle,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if id.is_null() {
        return false.to_glib();
    }

    let id: String = from_glib_none(id);
    // FIXME: return a proper error code to the public API
    rhandle.has_sub(&id).unwrap_or(false).to_glib()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_render_cairo_sub(
    handle: *const RsvgHandle,
    cr: *mut cairo_sys::cairo_t,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);
    let cr = from_glib_none(cr);
    let id: Option<String> = from_glib_none(id);

    match rhandle.render_cairo_sub(&cr, id.as_ref().map(String::as_str)) {
        Ok(()) => true.to_glib(),

        Err(_) => {
            // FIXME: return a proper error code to the public API
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_pixbuf_sub(
    handle: *const RsvgHandle,
    id: *const libc::c_char,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    let rhandle = get_rust_handle(handle);
    let id: Option<String> = from_glib_none(id);

    match rhandle.get_pixbuf_sub(id.as_ref().map(String::as_str)) {
        Ok(pixbuf) => pixbuf.to_glib_full(),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dimensions(
    handle: *const RsvgHandle,
    dimension_data: *mut RsvgDimensionData,
) {
    let rhandle = get_rust_handle(handle);

    *dimension_data = rhandle.get_dimensions_no_error();
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dimensions_sub(
    handle: *const RsvgHandle,
    dimension_data: *mut RsvgDimensionData,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_dimensions_sub(id.as_ref().map(String::as_str)) {
        Ok(dimensions) => {
            *dimension_data = dimensions;
            true.to_glib()
        }

        Err(_) => {
            let d = &mut *dimension_data;

            d.width = 0;
            d.height = 0;
            d.em = 0.0;
            d.ex = 0.0;

            // FIXME: return a proper error code to the public API
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_position_sub(
    handle: *const RsvgHandle,
    position_data: *mut RsvgPositionData,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_position_sub(id.as_ref().map(String::as_str)) {
        Ok(position) => {
            *position_data = position;
            true.to_glib()
        }

        Err(_) => {
            let p = &mut *position_data;

            p.x = 0;
            p.y = 0;

            // FIXME: return a proper error code to the public API
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new_with_flags(flags: u32) -> *const RsvgHandle {
    let obj: *mut gobject_sys::GObject =
        glib::Object::new(Handle::get_type(), &[("flags", &flags)])
            .unwrap()
            .to_glib_full();

    obj as *mut _
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new_from_file(
    filename: *const libc::c_char,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    // This API lets the caller pass a URI, or a file name in the operating system's
    // encoding.  So, first we'll see if it's UTF-8, and in that case, try the URL version.
    // Otherwise, we'll try building a path name.

    let cstr = CStr::from_ptr(filename);

    let file = cstr
        .to_str()
        .map_err(|_| ())
        .and_then(|utf8| Url::parse(utf8).map_err(|_| ()))
        .and_then(|url| Ok(gio::File::new_for_uri(url.as_str())))
        .unwrap_or_else(|_| gio::File::new_for_path(PathBuf::from_glib_none(filename)));

    rsvg_handle_rust_new_from_gfile_sync(file.to_glib_none().0, 0, ptr::null_mut(), error)
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new_from_gfile_sync(
    file: *mut gio_sys::GFile,
    flags: u32,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    let raw_handle = rsvg_handle_rust_new_with_flags(flags);

    let rhandle = get_rust_handle(raw_handle);

    let file = from_glib_none(file);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);

    match rhandle.construct_new_from_gfile_sync(&file, cancellable.as_ref()) {
        Ok(()) => raw_handle,

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            gobject_sys::g_object_unref(raw_handle as *mut _);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new_from_stream_sync(
    input_stream: *mut gio_sys::GInputStream,
    base_file: *mut gio_sys::GFile,
    flags: u32,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    let raw_handle = rsvg_handle_rust_new_with_flags(flags);

    let rhandle = get_rust_handle(raw_handle);

    let base_file: Option<gio::File> = from_glib_none(base_file);
    let stream = from_glib_none(input_stream);
    let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable);

    match rhandle.construct_read_stream_sync(&stream, base_file.as_ref(), cancellable.as_ref()) {
        Ok(()) => raw_handle,

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            gobject_sys::g_object_unref(raw_handle as *mut _);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new_from_data(
    data: *mut u8,
    len: usize,
    error: *mut *mut glib_sys::GError,
) -> *const RsvgHandle {
    // We create the MemoryInputStream without the gtk-rs binding because of this:
    //
    // - The binding doesn't provide _new_from_data().  All of the binding's ways to
    // put data into a MemoryInputStream involve copying the data buffer.
    //
    // - We can't use glib::Bytes from the binding either, for the same reason.
    //
    // - For now, we are using the other C-visible constructor, so we need a raw pointer to the
    //   stream, anyway.

    assert!(len <= std::isize::MAX as usize);
    let len = len as isize;

    let raw_stream = gio_sys::g_memory_input_stream_new_from_data(data, len, None);

    let ret = rsvg_handle_rust_new_from_stream_sync(
        raw_stream as *mut _,
        ptr::null_mut(), // base_file
        0,               // flags
        ptr::null_mut(), // cancellable
        error,
    );

    gobject_sys::g_object_unref(raw_stream as *mut _);
    ret
}

unsafe fn set_out_param<T: Copy>(
    out_has_param: *mut glib_sys::gboolean,
    out_param: *mut T,
    value: &Option<T>,
) {
    let has_value = if let Some(ref v) = *value {
        if !out_param.is_null() {
            *out_param = *v;
        }

        true
    } else {
        false
    };

    if !out_has_param.is_null() {
        *out_has_param = has_value.to_glib();
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_intrinsic_dimensions(
    handle: *mut RsvgHandle,
    out_has_width: *mut glib_sys::gboolean,
    out_width: *mut RsvgLength,
    out_has_height: *mut glib_sys::gboolean,
    out_height: *mut RsvgLength,
    out_has_viewbox: *mut glib_sys::gboolean,
    out_viewbox: *mut RsvgRectangle,
) {
    let rhandle = get_rust_handle(handle);

    if rhandle.check_is_loaded().is_err() {
        return;
    }

    let d = rhandle.get_intrinsic_dimensions();

    let w = d.width.map(|l| l.to_length());
    let h = d.width.map(|l| l.to_length());
    let r = d.vbox.map(RsvgRectangle::from);

    set_out_param(out_has_width, out_width, &w);
    set_out_param(out_has_height, out_height, &h);
    set_out_param(out_has_viewbox, out_viewbox, &r);
}
