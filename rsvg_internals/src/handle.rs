use std::cell::{Cell, Ref, RefCell};
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::slice;

use cairo::{self, ImageSurface, Status};
use cairo_sys;
use gdk_pixbuf::{Colorspace, Pixbuf, PixbufLoader, PixbufLoaderExt};
use gdk_pixbuf_sys;
use gio::File as GFile;
use gio_sys;
use glib::translate::*;
use glib_sys;
use libc;
use url::Url;

use allowed_url::AllowedUrl;
use css::{self, CssStyles};
use defs::Href;
use dpi::Dpi;
use drawing_ctx::{DrawingCtx, RsvgRectangle};
use error::{set_gerror, DefsLookupErrorKind, LoadingError, RenderingError};
use io;
use load::LoadContext;
use node::{Node, RsvgNode};
use rect::IRect;
use structure::NodeSvg;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
};
use svg::Svg;
use util::rsvg_g_warning;

// A *const RsvgHandle is just an opaque pointer we get from C
#[repr(C)]
pub struct RsvgHandle {
    _private: [u8; 0],
}

// Keep in sync with rsvg.h:RsvgDimensionData
#[repr(C)]
pub struct RsvgDimensionData {
    width: libc::c_int,
    height: libc::c_int,
    em: f64,
    ex: f64,
}

// Keep in sync with rsvg.h:RsvgPositionData
#[repr(C)]
pub struct RsvgPositionData {
    x: libc::c_int,
    y: libc::c_int,
}

/// Flags used during loading
///
/// We communicate these to/from the C code with a guint <-> u32,
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
}

impl LoadOptions {
    fn new(flags: LoadFlags, base_url: Option<Url>) -> LoadOptions {
        LoadOptions { flags, base_url }
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
    dpi: Dpi,
    base_url: RefCell<Option<Url>>,
    base_url_cstring: RefCell<Option<CString>>, // needed because the C api returns *const char
    svg: RefCell<Option<Rc<Svg>>>,
    load_flags: Cell<LoadFlags>,
    load_state: Cell<LoadState>,
    load: RefCell<Option<LoadContext>>,
    size_callback: RefCell<SizeCallback>,
    in_loop: Cell<bool>,
    is_testing: Cell<bool>,
}

impl Handle {
    fn new() -> Handle {
        Handle {
            dpi: Dpi::default(),
            base_url: RefCell::new(None),
            base_url_cstring: RefCell::new(None),
            svg: RefCell::new(None),
            load_flags: Cell::new(LoadFlags::default()),
            load_state: Cell::new(LoadState::Start),
            load: RefCell::new(None),
            size_callback: RefCell::new(SizeCallback::new()),
            in_loop: Cell::new(false),
            is_testing: Cell::new(false),
        }
    }

    fn set_base_url(&self, url: &str) {
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

    pub fn read_stream_sync(
        &mut self,
        handle: *mut RsvgHandle,
        stream: gio::InputStream,
        cancellable: Option<gio::Cancellable>,
    ) -> Result<(), LoadingError> {
        self.load_state.set(LoadState::Loading);

        let svg = Svg::load_from_stream(&self.load_options(), handle, stream, cancellable)
            .map_err(|e| {
                self.load_state.set(LoadState::ClosedError);
                e
            })?;

        *self.svg.borrow_mut() = Some(Rc::new(svg));
        self.load_state.set(LoadState::ClosedOk);
        Ok(())
    }

    fn load_options(&self) -> LoadOptions {
        LoadOptions::new(self.load_flags.get(), self.base_url.borrow().clone())
    }

    pub fn write(&mut self, handle: *mut RsvgHandle, buf: &[u8]) {
        assert!(
            self.load_state.get() == LoadState::Start
                || self.load_state.get() == LoadState::Loading
        );

        if self.load_state.get() == LoadState::Start {
            self.load_state.set(LoadState::Loading);

            self.load = RefCell::new(Some(LoadContext::new(handle, self.load_options())));
        }

        assert!(self.load_state.get() == LoadState::Loading);

        self.load.borrow_mut().as_mut().unwrap().write(buf);
    }

    pub fn close(&mut self) -> Result<(), LoadingError> {
        let load_state = self.load_state.get();

        let res = match load_state {
            LoadState::Start => {
                self.load_state.set(LoadState::ClosedError);
                Err(LoadingError::NoDataPassedToParser)
            }

            LoadState::Loading => self
                .close_internal()
                .and_then(|_| {
                    self.load_state.set(LoadState::ClosedOk);
                    Ok(())
                })
                .map_err(|e| {
                    self.load_state.set(LoadState::ClosedError);
                    e
                }),

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

    fn close_internal(&mut self) -> Result<(), LoadingError> {
        let mut r = self.load.borrow_mut();
        let mut load = r.take().unwrap();

        let mut xml = load.close()?;

        xml.validate_tree()?;

        *self.svg.borrow_mut() = Some(Rc::new(xml.steal_result()));
        Ok(())
    }

    fn cascade(&mut self) {
        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();

        svg.tree.cascade();
    }

    fn create_drawing_ctx_for_node(
        &mut self,
        cr: &cairo::Context,
        dimensions: &RsvgDimensionData,
        node: Option<&RsvgNode>,
    ) -> DrawingCtx {
        let mut draw_ctx = DrawingCtx::new(
            self.svg.borrow().as_ref().unwrap().clone(),
            cr,
            f64::from(dimensions.width),
            f64::from(dimensions.height),
            dimensions.em,
            dimensions.ex,
            self.dpi.clone(),
            self.is_testing.get(),
        );

        if let Some(node) = node {
            draw_ctx.add_node_and_ancestors_to_stack(node);
        }

        self.cascade();

        draw_ctx
    }

    fn get_dimensions(
        &mut self,
        handle: *mut RsvgHandle,
    ) -> Result<RsvgDimensionData, RenderingError> {
        let dimensions = unsafe {
            let mut dimensions = mem::zeroed();
            rsvg_handle_get_dimensions(handle, &mut dimensions);
            dimensions
        };

        if dimensions.width == 0 || dimensions.height == 0 {
            Err(RenderingError::SvgHasNoSize)
        } else {
            Ok(dimensions)
        }
    }

    fn get_node_geometry(
        &mut self,
        handle: *mut RsvgHandle,
        node: &RsvgNode,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let dimensions = self.get_dimensions(handle)?;
        let target = ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);
        let mut draw_ctx = self.create_drawing_ctx_for_node(&cr, &dimensions, Some(node));
        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();
        let root = svg.tree.root();

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

    fn get_geometry_sub(
        &mut self,
        handle: *mut RsvgHandle,
        id: Option<&str>,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let root = {
            let svg_ref = self.svg.borrow();
            let svg = svg_ref.as_ref().unwrap();

            svg.tree.root()
        };

        let (node, is_root) = if let Some(id) = id {
            let n = self.lookup_node(id).map_err(RenderingError::InvalidId)?;
            let is_root = Rc::ptr_eq(&n, &root);
            (n, is_root)
        } else {
            (root, true)
        };

        if is_root {
            if let Some((root_width, root_height)) =
                node.with_impl(|svg: &NodeSvg| svg.get_size(&self.dpi))
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

        self.get_node_geometry(handle, &node)
    }

    fn lookup_node(&mut self, id: &str) -> Result<RsvgNode, DefsLookupErrorKind> {
        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();

        let href = Href::with_fragment(id).map_err(DefsLookupErrorKind::HrefError)?;

        match href {
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

            _ => unreachable!(), // we explicitly requested a with_fragment after all
        }
    }

    fn has_sub(&mut self, name: &str) -> bool {
        // FIXME: return a proper error; only NotFound should map to false
        self.lookup_node(name).is_ok()
    }

    fn render_cairo_sub(
        &mut self,
        handle: *mut RsvgHandle,
        cr: &cairo::Context,
        id: Option<&str>,
    ) -> Result<(), RenderingError> {
        let status = cr.status();
        if status != Status::Success {
            let msg = format!(
                "cannot render on a cairo_t with a failure status (status={:?})",
                status,
            );

            rsvg_g_warning(&msg);
            return Err(RenderingError::Cairo(status));
        }

        let node = if let Some(id) = id {
            Some(self.lookup_node(id).map_err(RenderingError::InvalidId)?)
        } else {
            None
        };

        let dimensions = self.get_dimensions(handle)?;

        cr.save();

        let mut draw_ctx = self.create_drawing_ctx_for_node(cr, &dimensions, node.as_ref());

        let svg_ref = self.svg.borrow();
        let svg = svg_ref.as_ref().unwrap();
        let root = svg.tree.root();

        let res = draw_ctx.draw_node_from_stack(&root.get_cascaded_values(), &root, false);

        cr.restore();

        res
    }

    fn get_pixbuf_sub(
        &mut self,
        handle: *mut RsvgHandle,
        id: Option<&str>,
    ) -> Result<Pixbuf, RenderingError> {
        let dimensions = self.get_dimensions(handle)?;

        let surface =
            ImageSurface::create(cairo::Format::ARgb32, dimensions.width, dimensions.height)?;

        {
            let cr = cairo::Context::new(&surface);
            self.render_cairo_sub(handle, &cr, id)?;
        }

        let surface = SharedImageSurface::new(surface, SurfaceType::SRgb)?;

        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: dimensions.width,
            y1: dimensions.height,
        };

        let pixbuf = Pixbuf::new(
            Colorspace::Rgb,
            true,
            8,
            dimensions.width,
            dimensions.height,
        );

        for (x, y, pixel) in Pixels::new(&surface, bounds) {
            let (r, g, b, a) = if pixel.a == 0 {
                (0, 0, 0, 0)
            } else {
                let pixel = pixel.unpremultiply();
                (pixel.r, pixel.g, pixel.b, pixel.a)
            };

            pixbuf.put_pixel(x as i32, y as i32, r, g, b, a);
        }

        Ok(pixbuf)
    }
}

// Keep these in sync with rsvg.h:RsvgHandleFlags
const RSVG_HANDLE_FLAG_UNLIMITED: u32 = 1 << 0;
const RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA: u32 = 1 << 1;

pub fn get_load_options(handle: *const RsvgHandle) -> LoadOptions {
    let rhandle = get_rust_handle(handle);

    rhandle.load_options()
}

impl LoadFlags {
    pub fn from_flags(flags: u32) -> Self {
        LoadFlags {
            unlimited_size: (flags & RSVG_HANDLE_FLAG_UNLIMITED) != 0,
            keep_image_data: (flags & RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA) != 0,
        }
    }

    fn to_flags(&self) -> u32 {
        let mut flags = 0;

        if self.unlimited_size {
            flags |= RSVG_HANDLE_FLAG_UNLIMITED;
        }

        if self.keep_image_data {
            flags |= RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA;
        }

        flags
    }
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_handle_new_from_gfile_sync(
        file: *const gio_sys::GFile,
        flags: u32,
        cancellable: *const gio_sys::GCancellable,
        error: *mut *mut glib_sys::GError,
    ) -> *mut RsvgHandle;

    fn rsvg_handle_get_rust(handle: *const RsvgHandle) -> *mut Handle;

    fn rsvg_handle_get_dimensions(handle: *mut RsvgHandle, dimensions: *mut RsvgDimensionData);
}

// Looks up a node by its id.
pub fn lookup_fragment_id(handle: *const RsvgHandle, id: &str) -> Option<Rc<Node>> {
    let rhandle = get_rust_handle(handle);

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();

    svg.lookup_node_by_id(id)
}

pub fn load_extern(load_options: &LoadOptions, aurl: &AllowedUrl) -> Result<*const RsvgHandle, ()> {
    unsafe {
        let file = GFile::new_for_uri(aurl.url().as_str());

        let res = rsvg_handle_new_from_gfile_sync(
            file.to_glib_none().0,
            load_options.flags.to_flags(),
            ptr::null(),
            ptr::null_mut(),
        );

        if res.is_null() {
            Err(())
        } else {
            Ok(res)
        }
    }
}

pub fn get_base_url<'a>(handle: *const RsvgHandle) -> Ref<'a, Option<Url>> {
    let rhandle = get_rust_handle(handle);

    rhandle.base_url.borrow()
}

pub struct BinaryData {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn load_image_to_surface(
    load_options: &LoadOptions,
    url: &str,
) -> Result<ImageSurface, LoadingError> {
    let aurl = AllowedUrl::from_href(url, load_options.base_url.as_ref())
        .map_err(|_| LoadingError::BadUrl)?;

    let data = io::acquire_data(&aurl, None)?;

    if data.data.len() == 0 {
        return Err(LoadingError::EmptyData);
    }

    let loader = if let Some(ref content_type) = data.content_type {
        PixbufLoader::new_with_mime_type(content_type)?
    } else {
        PixbufLoader::new()
    };

    loader.write(&data.data)?;
    loader.close()?;

    let pixbuf = loader.get_pixbuf().ok_or(LoadingError::Unknown)?;

    let surface = SharedImageSurface::from_pixbuf(&pixbuf)?.into_image_surface()?;

    if load_options.flags.keep_image_data {
        if let Some(mime_type) = data.content_type {
            extern "C" {
                fn cairo_surface_set_mime_data(
                    surface: *mut cairo_sys::cairo_surface_t,
                    mime_type: *const libc::c_char,
                    data: *mut libc::c_char,
                    length: libc::c_ulong,
                    destroy: cairo_sys::cairo_destroy_func_t,
                    closure: *mut libc::c_void,
                ) -> Status;
            }

            let data_ptr = ToGlibContainerFromSlice::to_glib_full_from_slice(&data.data);

            unsafe {
                let status = cairo_surface_set_mime_data(
                    surface.to_glib_none().0,
                    mime_type.to_glib_none().0,
                    data_ptr as *mut _,
                    data.data.len() as libc::c_ulong,
                    Some(glib_sys::g_free),
                    data_ptr as *mut _,
                );

                if status != Status::Success {
                    return Err(LoadingError::Cairo(status));
                }
            }
        }
    }

    Ok(surface)
}

// This function just slurps CSS data from a possibly-relative href
// and parses it.  We'll move it to a better place in the end.
pub fn load_css(css_styles: &mut CssStyles, aurl: &AllowedUrl) -> Result<(), LoadingError> {
    io::acquire_data(aurl, None)
        .and_then(|data| {
            let BinaryData {
                data: bytes,
                content_type,
            } = data;

            if content_type.as_ref().map(String::as_ref) == Some("text/css") {
                Ok(bytes)
            } else {
                rsvg_log!("\"{}\" is not of type text/css; ignoring", aurl);
                Err(LoadingError::BadCss)
            }
        })
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| {
                rsvg_log!(
                    "\"{}\" does not contain valid UTF-8 CSS data; ignoring",
                    aurl
                );
                LoadingError::BadCss
            })
        })
        .and_then(|utf8| {
            css::parse_into_css_styles(css_styles, Some(aurl.url().clone()), &utf8);
            Ok(()) // FIXME: return CSS parsing errors
        })
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new() -> *mut Handle {
    Box::into_raw(Box::new(Handle::new()))
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_free(raw_handle: *mut Handle) {
    assert!(!raw_handle.is_null());
    Box::from_raw(raw_handle);
}

fn get_rust_handle<'a>(handle: *const RsvgHandle) -> &'a mut Handle {
    unsafe { &mut *(rsvg_handle_get_rust(handle) as *mut Handle) }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_base_url(
    raw_handle: *const Handle,
    uri: *const libc::c_char,
) {
    let handle = &*raw_handle;

    if handle.load_state.get() != LoadState::Start {
        rsvg_g_warning("Please set the base file or URI before loading any data into RsvgHandle");
        return;
    }

    assert!(!uri.is_null());
    let uri: String = from_glib_none(uri);

    handle.set_base_url(&uri);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_base_gfile(
    raw_handle: *const Handle,
) -> *mut gio_sys::GFile {
    let handle = &*raw_handle;

    match *handle.base_url.borrow() {
        None => ptr::null_mut(),
        Some(ref url) => GFile::new_for_uri(url.as_str()).to_glib_full(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_base_url(
    raw_handle: *const Handle,
) -> *const libc::c_char {
    let handle = &*raw_handle;

    match *handle.base_url_cstring.borrow() {
        None => ptr::null(),
        Some(ref url) => url.as_ptr(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_dpi_x(raw_handle: *const Handle, dpi_x: f64) {
    let handle = &*(raw_handle as *const Handle);

    handle.dpi.set_x(dpi_x);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dpi_x(raw_handle: *const Handle) -> f64 {
    let handle = &*(raw_handle as *const Handle);

    handle.dpi.x()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_dpi_y(raw_handle: *const Handle, dpi_y: f64) {
    let handle = &*(raw_handle as *const Handle);

    handle.dpi.set_y(dpi_y);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dpi_y(raw_handle: *const Handle) -> f64 {
    let handle = &*(raw_handle as *const Handle);

    handle.dpi.y()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_flags(raw_handle: *const Handle) -> u32 {
    let rhandle = &*raw_handle;

    rhandle.load_flags.get().to_flags()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_flags(raw_handle: *const Handle, flags: u32) {
    let rhandle = &*raw_handle;

    rhandle.load_flags.set(LoadFlags::from_flags(flags));
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_size_callback(
    raw_handle: *mut Handle,
    size_func: RsvgSizeFunc,
    user_data: glib_sys::gpointer,
    destroy_notify: glib_sys::GDestroyNotify,
) {
    let rhandle = &mut *raw_handle;

    *rhandle.size_callback.borrow_mut() = SizeCallback {
        size_func,
        user_data,
        destroy_notify,
    };
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_testing(
    raw_handle: *const Handle,
    testing: glib_sys::gboolean,
) {
    let rhandle = &*raw_handle;

    rhandle.is_testing.set(from_glib(testing));
}

fn is_loaded(handle: &Handle) -> bool {
    match handle.load_state.get() {
        LoadState::Start => {
            rsvg_g_warning("RsvgHandle has not been loaded");
            false
        }

        LoadState::Loading => {
            rsvg_g_warning("RsvgHandle is still loading; call rsvg_handle_close() first");
            false
        }

        LoadState::ClosedOk => true,

        LoadState::ClosedError => {
            rsvg_g_warning(
                "RsvgHandle could not read or parse the SVG; did you check for errors during the \
                 loading stage?",
            );
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_read_stream_sync(
    handle: *mut RsvgHandle,
    stream: *mut gio_sys::GInputStream,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if rhandle.load_state.get() != LoadState::Start {
        rsvg_g_warning(
            "handle must not be already loaded in order to call rsvg_handle_read_stream_sync()",
        );
        return false.to_glib();
    }

    let stream = from_glib_none(stream);
    let cancellable = from_glib_none(cancellable);

    match rhandle.read_stream_sync(handle, stream, cancellable) {
        Ok(()) => true.to_glib(),

        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_write(
    handle: *mut RsvgHandle,
    buf: *const u8,
    count: usize,
) {
    let rhandle = get_rust_handle(handle);

    let load_state = rhandle.load_state.get();

    if !(load_state == LoadState::Start || load_state == LoadState::Loading) {
        rsvg_g_warning("handle must not be closed in order to write to it");
        return;
    }

    let buffer = slice::from_raw_parts(buf, count);

    rhandle.write(handle, buffer);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_close(
    handle: *mut RsvgHandle,
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
    handle: *mut RsvgHandle,
    out_ink_rect: *mut RsvgRectangle,
    out_logical_rect: *mut RsvgRectangle,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if !is_loaded(rhandle) {
        return false.to_glib();
    }

    let id: Option<String> = from_glib_none(id);

    match rhandle.get_geometry_sub(handle, id.as_ref().map(String::as_str)) {
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
    handle: *mut RsvgHandle,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if !is_loaded(rhandle) {
        return false.to_glib();
    }

    if id.is_null() {
        return false.to_glib();
    }

    let id: String = from_glib_none(id);
    rhandle.has_sub(&id).to_glib()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_render_cairo_sub(
    handle: *mut RsvgHandle,
    cr: *mut cairo_sys::cairo_t,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);
    let cr = from_glib_none(cr);
    let id: Option<String> = from_glib_none(id);

    if !is_loaded(rhandle) {
        return false.to_glib();
    }

    match rhandle.render_cairo_sub(handle, &cr, id.as_ref().map(String::as_str)) {
        Ok(()) => true.to_glib(),

        Err(_) => {
            // FIXME: return a proper error code to the public API
            false.to_glib()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_pixbuf_sub(
    handle: *mut RsvgHandle,
    id: *const libc::c_char,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    let rhandle = get_rust_handle(handle);
    let id: Option<String> = from_glib_none(id);

    if !is_loaded(rhandle) {
        return ptr::null_mut();
    }

    match rhandle.get_pixbuf_sub(handle, id.as_ref().map(String::as_str)) {
        Ok(pixbuf) => pixbuf.to_glib_full(),
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dimensions(
    handle: *mut RsvgHandle,
    dimension_data: *mut RsvgDimensionData,
) {
    let rhandle = get_rust_handle(handle);

    if !is_loaded(rhandle) {
        return;
    }

    // This function is probably called from the cairo_render functions.
    // To prevent an infinite loop we are saving the state.
    if rhandle.in_loop.get() {
        // Called within the size function, so return a standard size
        (*dimension_data).width = 1;
        (*dimension_data).height = 1;
        (*dimension_data).em = 1.0;
        (*dimension_data).ex = 1.0;
        return;
    }

    rhandle.in_loop.set(true);
    rsvg_handle_rust_get_dimensions_sub(handle, dimension_data, ptr::null());
    rhandle.in_loop.set(false);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_dimensions_sub(
    handle: *mut RsvgHandle,
    dimension_data: *mut RsvgDimensionData,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if !is_loaded(rhandle) {
        return false.to_glib();
    }

    let mut ink_r = RsvgRectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    let res = rsvg_handle_rust_get_geometry_sub(handle, &mut ink_r, ptr::null_mut(), id);
    if from_glib(res) {
        let (w, h) = rhandle
            .size_callback
            .borrow()
            .call(ink_r.width as libc::c_int, ink_r.height as libc::c_int);

        (*dimension_data).width = w;
        (*dimension_data).height = h;
        (*dimension_data).em = ink_r.width;
        (*dimension_data).ex = ink_r.height;
    } else {
        (*dimension_data).width = 0;
        (*dimension_data).height = 0;
        (*dimension_data).em = 0.0;
        (*dimension_data).ex = 0.0;
    }

    res
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_position_sub(
    handle: *mut RsvgHandle,
    position: *mut RsvgPositionData,
    id: *const libc::c_char,
) -> glib_sys::gboolean {
    let rhandle = get_rust_handle(handle);

    if !is_loaded(rhandle) {
        return false.to_glib();
    }

    // Short-cut when no id is given
    if id.is_null() || *id == 0 {
        (*position).x = 0;
        (*position).y = 0;
        return true.to_glib();
    }

    let mut ink_r = RsvgRectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    let res = rsvg_handle_rust_get_geometry_sub(handle, &mut ink_r, ptr::null_mut(), id);
    if from_glib(res) {
        (*position).x = ink_r.x as libc::c_int;
        (*position).y = ink_r.y as libc::c_int;

        let width = ink_r.width as libc::c_int;
        let height = ink_r.height as libc::c_int;

        rhandle.size_callback.borrow().call(width, height);
    } else {
        (*position).x = 0;
        (*position).y = 0;
    }

    res
}
