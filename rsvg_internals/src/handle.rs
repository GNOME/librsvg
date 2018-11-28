use std::cell::{Ref, RefCell};
use std::error::Error;
use std::ptr;

use cairo::{ImageSurface, Status};
use cairo_sys;
use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use gio::{Cancellable, File as GFile, InputStream};
use gio_sys;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use url::Url;

use allowed_url::AllowedUrl;
use css::{self, CssStyles};
use defs::{Defs, RsvgDefs};
use error::{set_gerror, LoadingError, RsvgError};
use io;
use surface_utils::shared_surface::SharedImageSurface;

pub enum RsvgHandle {}

pub enum RsvgHandleRust {}

struct Handle {
    base_url: RefCell<Option<Url>>,
}

impl Handle {
    fn new() -> Handle {
        Handle {
            base_url: RefCell::new(None),
        }
    }
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_handle_get_defs(handle: *const RsvgHandle) -> *const RsvgDefs;

    fn rsvg_handle_load_extern(
        handle: *const RsvgHandle,
        href: *const libc::c_char,
    ) -> *const RsvgHandle;

    fn rsvg_handle_keep_image_data(handle: *const RsvgHandle) -> glib_sys::gboolean;

    fn rsvg_load_handle_xml_xinclude(
        handle: *mut RsvgHandle,
        href: *const libc::c_char,
    ) -> glib_sys::gboolean;

    fn rsvg_handle_get_rust(handle: *const RsvgHandle) -> *mut RsvgHandleRust;

    fn rsvg_handle_get_cancellable(handle: *const RsvgHandle) -> *mut gio_sys::GCancellable;
}

pub fn get_defs<'a>(handle: *const RsvgHandle) -> &'a mut Defs {
    unsafe {
        let d = rsvg_handle_get_defs(handle);
        &mut *(d as *mut Defs)
    }
}

pub fn load_extern(handle: *const RsvgHandle, uri: &str) -> Result<*const RsvgHandle, ()> {
    unsafe {
        let res = rsvg_handle_load_extern(handle, uri.to_glib_none().0);

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

fn get_cancellable<'a>(handle: *const RsvgHandle) -> Option<Cancellable> {
    unsafe { from_glib_borrow(rsvg_handle_get_cancellable(handle)) }
}

pub struct BinaryData {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn acquire_data(handle: *mut RsvgHandle, aurl: &AllowedUrl) -> Result<BinaryData, glib::Error> {
    io::acquire_data(aurl, get_cancellable(handle).as_ref())
        .map_err(|_| glib::Error::new(RsvgError, "FIXME"))
}

pub fn acquire_stream(
    handle: *mut RsvgHandle,
    aurl: &AllowedUrl,
) -> Result<InputStream, glib::Error> {
    io::acquire_stream(&aurl, get_cancellable(handle).as_ref())
        .map_err(|_| glib::Error::new(RsvgError, "FIXME"))
}

fn keep_image_data(handle: *const RsvgHandle) -> bool {
    unsafe { from_glib(rsvg_handle_keep_image_data(handle)) }
}

pub fn image_surface_new_from_href(
    handle: *mut RsvgHandle,
    href_str: &str,
) -> Result<ImageSurface, LoadingError> {
    let rhandle = get_rust_handle(handle);

    let aurl = AllowedUrl::from_href(href_str, rhandle.base_url.borrow().as_ref())
        .map_err(|_| glib::Error::new(RsvgError, "FIXME"))?;

    let data = acquire_data(handle, &aurl)?;

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

    if keep_image_data(handle) {
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

// FIXME: distinguish between "file not found" and "invalid XML"
pub fn load_xml_xinclude(handle: *mut RsvgHandle, aurl: &AllowedUrl) -> bool {
    let href = aurl.url().as_str();

    unsafe { from_glib(rsvg_load_handle_xml_xinclude(handle, href.to_glib_none().0)) }
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

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_new() -> *mut RsvgHandleRust {
    Box::into_raw(Box::new(Handle::new())) as *mut RsvgHandleRust
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_free(raw_handle: *mut RsvgHandleRust) {
    assert!(!raw_handle.is_null());

    Box::from_raw(raw_handle as *mut Handle);
}

fn get_rust_handle<'a>(handle: *const RsvgHandle) -> &'a mut Handle {
    unsafe { &mut *(rsvg_handle_get_rust(handle) as *mut Handle) }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_handle_rust_set_base_url(
    raw_handle: *const RsvgHandleRust,
    uri: *const libc::c_char,
) {
    let handle = &*(raw_handle as *const Handle);

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
    raw_handle: *const RsvgHandleRust,
) -> *mut gio_sys::GFile {
    let handle = &*(raw_handle as *const Handle);

    match *handle.base_url.borrow() {
        None => ptr::null_mut(),

        Some(ref url) => GFile::new_for_uri(url.as_str()).to_glib_full(),
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
