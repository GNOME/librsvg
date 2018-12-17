use std::cell::{Cell, Ref, RefCell};
use std::error::Error;
use std::ptr;
use std::rc::Rc;

use cairo::{ImageSurface, Status};
use cairo_sys;
use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use gio::{File as GFile, InputStream};
use gio_sys;
use glib::translate::*;
use glib_sys;
use libc;
use url::Url;

use allowed_url::AllowedUrl;
use css::{self, CssStyles};
use defs::{Fragment, Href};
use dpi::Dpi;
use error::{set_gerror, LoadingError};
use io;
use node::{box_node, Node, RsvgNode};
use surface_utils::shared_surface::SharedImageSurface;
use svg::Svg;
use util::rsvg_g_warning;
use xml::XmlState;

// A *const RsvgHandle is just an opaque pointer we get from C
#[repr(C)]
pub struct RsvgHandle {
    _private: [u8; 0],
}

/// Flags used during loading
///
/// We communicate these to/from the C code with a guint <-> u32,
/// and this struct provides to_flags() and from_flags() methods.
#[derive(Default, Copy, Clone)]
pub struct LoadOptions {
    /// Whether to turn off size limits in libxml2
    pub unlimited_size: bool,

    /// Whether to keep original (undecoded) image data to embed in Cairo PDF surfaces
    pub keep_image_data: bool,
}

pub struct Handle {
    dpi: Dpi,
    base_url: RefCell<Option<Url>>,
    svg: RefCell<Option<Svg>>,
    load_options: Cell<LoadOptions>,
}

impl Handle {
    fn new() -> Handle {
        Handle {
            dpi: Dpi::default(),
            base_url: RefCell::new(None),
            svg: RefCell::new(None),
            load_options: Cell::new(LoadOptions::default()),
        }
    }
}

// Keep these in sync with rsvg.h:RsvgHandleFlags
const RSVG_HANDLE_FLAG_UNLIMITED: u32 = 1 << 0;
const RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA: u32 = 1 << 1;

pub fn get_load_options(handle: *const RsvgHandle) -> LoadOptions {
    let rhandle = get_rust_handle(handle);
    rhandle.load_options.get()
}

impl LoadOptions {
    pub fn from_flags(flags: u32) -> Self {
        LoadOptions {
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
}

pub fn lookup_node(handle: *const RsvgHandle, fragment: &Fragment) -> Option<Rc<Node>> {
    let rhandle = get_rust_handle(handle);

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();
    let mut defs_ref = svg.defs.borrow_mut();

    defs_ref.lookup(handle, fragment)
}

// Looks up a node by its id.
//
// Note that this ignores the Fragment's url; it only uses the fragment identifier.
pub fn lookup_fragment_id(handle: *const RsvgHandle, fragment: &Fragment) -> Option<Rc<Node>> {
    let rhandle = get_rust_handle(handle);

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();
    let defs_ref = svg.defs.borrow();

    defs_ref.lookup_fragment_id(fragment.fragment())
}

pub fn load_extern(handle: *const RsvgHandle, aurl: &AllowedUrl) -> Result<*const RsvgHandle, ()> {
    unsafe {
        let rhandle = get_rust_handle(handle);

        let file = GFile::new_for_uri(aurl.url().as_str());

        let res = rsvg_handle_new_from_gfile_sync(
            file.to_glib_none().0,
            rhandle.load_options.get().to_flags(),
            ptr::null(),
            ptr::null_mut(),
        );

        if res.is_null() {
            Err(())
        } else {
            let rhandle = get_rust_handle(handle);

            let svg_ref = rhandle.svg.borrow();
            let svg = svg_ref.as_ref().unwrap();

            svg.tree.cascade();

            Ok(res)
        }
    }
}

pub fn get_dpi<'a>(handle: *const RsvgHandle) -> &'a Dpi {
    let rhandle = get_rust_handle(handle);

    &rhandle.dpi
}

pub fn get_base_url<'a>(handle: *const RsvgHandle) -> Ref<'a, Option<Url>> {
    let rhandle = get_rust_handle(handle);

    rhandle.base_url.borrow()
}

pub struct BinaryData {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn acquire_data(
    _handle: *mut RsvgHandle,
    aurl: &AllowedUrl,
) -> Result<BinaryData, LoadingError> {
    io::acquire_data(aurl, None)
}

pub fn acquire_stream(
    _handle: *mut RsvgHandle,
    aurl: &AllowedUrl,
) -> Result<InputStream, LoadingError> {
    io::acquire_stream(&aurl, None)
}

pub fn load_image_to_surface(
    handle: *mut RsvgHandle,
    aurl: &AllowedUrl,
) -> Result<ImageSurface, LoadingError> {
    let rhandle = get_rust_handle(handle);

    let data = acquire_data(handle, aurl)?;

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

    if rhandle.load_options.get().keep_image_data {
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
pub fn load_css(css_styles: &mut CssStyles, handle: *mut RsvgHandle, href_str: &str) {
    let rhandle = get_rust_handle(handle);

    let aurl = match AllowedUrl::from_href(href_str, rhandle.base_url.borrow().as_ref()) {
        Ok(a) => a,
        Err(_) => {
            rsvg_log!("Could not load \"{}\" for CSS data", href_str);
            // FIXME: report errors; this should be a fatal error
            return;
        }
    };

    if let Ok(data) = acquire_data(handle, &aurl) {
        let BinaryData {
            data: bytes,
            content_type,
        } = data;

        if content_type.as_ref().map(String::as_ref) != Some("text/css") {
            rsvg_log!("\"{}\" is not of type text/css; ignoring", href_str);
            // FIXME: report errors
            return;
        }

        if let Ok(utf8) = String::from_utf8(bytes) {
            css::parse_into_css_styles(css_styles, handle, &utf8);
        } else {
            rsvg_log!(
                "\"{}\" does not contain valid UTF-8 CSS data; ignoring",
                href_str
            );
            // FIXME: report errors
            return;
        }
    } else {
        rsvg_log!("Could not load \"{}\" for CSS data", href_str);
        // FIXME: report errors from not being to acquire data; this should be a fatal error
    }
}

pub fn get_svg<'a>(handle: *const RsvgHandle) -> Ref<'a, Option<Svg>> {
    let rhandle = get_rust_handle(handle);

    rhandle.svg.borrow()
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

pub fn get_rust_handle<'a>(handle: *const RsvgHandle) -> &'a mut Handle {
    unsafe { &mut *(rsvg_handle_get_rust(handle) as *mut Handle) }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_base_url(
    raw_handle: *const Handle,
    uri: *const libc::c_char,
) {
    let handle = &*raw_handle;

    assert!(!uri.is_null());
    let uri: String = from_glib_none(uri);

    let url = match Url::parse(&uri) {
        Ok(u) => u,

        Err(e) => {
            rsvg_log!(
                "not setting base_uri to \"{}\" since it is invalid: {}",
                uri,
                e
            );
            return;
        }
    };

    rsvg_log!("setting base_uri to \"{}\"", url);
    *handle.base_url.borrow_mut() = Some(url);
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
pub unsafe extern "C" fn rsvg_handle_defs_lookup(
    handle: *const RsvgHandle,
    name: *const libc::c_char,
) -> *const RsvgNode {
    assert!(!name.is_null());

    let rhandle = get_rust_handle(handle);

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();

    let mut defs = svg.defs.borrow_mut();

    let name: String = from_glib_none(name);

    let r = Href::with_fragment(&name);
    if r.is_err() {
        return ptr::null();
    }

    match r.unwrap() {
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
                return ptr::null();
            }

            match defs.lookup(handle, &fragment) {
                Some(n) => box_node(n),
                None => ptr::null(),
            }
        }

        _ => unreachable!(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_acquire_data(
    handle: *mut RsvgHandle,
    href_str: *const libc::c_char,
    out_len: *mut usize,
    error: *mut *mut glib_sys::GError,
) -> *mut libc::c_char {
    assert!(!href_str.is_null());
    assert!(!out_len.is_null());

    let href_str: String = from_glib_none(href_str);

    let rhandle = get_rust_handle(handle);

    let aurl = match AllowedUrl::from_href(&href_str, rhandle.base_url.borrow().as_ref()) {
        Ok(a) => a,
        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            return ptr::null_mut();
        }
    };

    match acquire_data(handle, &aurl) {
        Ok(binary) => {
            if !error.is_null() {
                *error = ptr::null_mut();
            }

            *out_len = binary.data.len();
            io::binary_data_to_glib(&binary, ptr::null_mut(), out_len)
        }

        Err(e) => {
            set_gerror(error, 0, e.description());
            *out_len = 0;
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_acquire_stream(
    handle: *mut RsvgHandle,
    href_str: *const libc::c_char,
    error: *mut *mut glib_sys::GError,
) -> *mut gio_sys::GInputStream {
    assert!(!href_str.is_null());

    let href_str: String = from_glib_none(href_str);

    let rhandle = get_rust_handle(handle);

    let aurl = match AllowedUrl::from_href(&href_str, rhandle.base_url.borrow().as_ref()) {
        Ok(a) => a,
        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            return ptr::null_mut();
        }
    };

    match acquire_stream(handle, &aurl) {
        Ok(stream) => {
            if !error.is_null() {
                *error = ptr::null_mut();
            }

            stream.to_glib_full()
        }

        Err(e) => {
            set_gerror(error, 0, e.description());
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_steal_result(
    raw_handle: *const Handle,
    raw_xml_state: *mut XmlState,
) {
    let handle = &*raw_handle;
    let xml = &mut *raw_xml_state;

    *handle.svg.borrow_mut() = Some(xml.steal_result());
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_cascade(raw_handle: *const Handle) {
    let rhandle = &*raw_handle;

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();

    svg.tree.cascade();
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_root(raw_handle: *const Handle) -> *const RsvgNode {
    let rhandle = &*raw_handle;

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();

    box_node(svg.tree.root())
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_node_is_root(
    raw_handle: *const Handle,
    node: *mut RsvgNode,
) -> glib_sys::gboolean {
    let rhandle = &*raw_handle;

    assert!(!node.is_null());
    let node: &RsvgNode = &*node;

    let svg_ref = rhandle.svg.borrow();
    let svg = svg_ref.as_ref().unwrap();

    Rc::ptr_eq(&svg.tree.root(), node).to_glib()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_get_flags(raw_handle: *const Handle) -> u32 {
    let rhandle = &*raw_handle;

    rhandle.load_options.get().to_flags()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_flags(raw_handle: *const Handle, flags: u32) {
    let rhandle = &*raw_handle;

    rhandle.load_options.set(LoadOptions::from_flags(flags));
}
