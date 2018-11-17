use std::ptr;

use glib;
use glib::translate::*;
use glib_sys;
use libc;

use css::{CssStyles, RsvgCssStyles};
use defs::{Defs, RsvgDefs};

pub enum RsvgHandle {}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_handle_get_defs(handle: *const RsvgHandle) -> *const RsvgDefs;

    fn rsvg_handle_resolve_uri(
        handle: *const RsvgHandle,
        uri: *const libc::c_char,
    ) -> *const libc::c_char;

    fn rsvg_handle_load_extern(
        handle: *const RsvgHandle,
        uri: *const libc::c_char,
    ) -> *const RsvgHandle;

    fn rsvg_handle_get_css_styles(handle: *const RsvgHandle) -> *mut RsvgCssStyles;

    fn _rsvg_handle_acquire_data(
        handle: *mut RsvgHandle,
        url: *const libc::c_char,
        out_content_type: *mut *mut libc::c_char,
        out_len: *mut usize,
        error: *mut *mut glib_sys::GError,
    ) -> *mut u8;

    fn rsvg_load_handle_xml_xinclude(
        handle: *mut RsvgHandle,
        url: *const libc::c_char,
    ) -> glib_sys::gboolean;
}

pub fn get_defs<'a>(handle: *const RsvgHandle) -> &'a mut Defs {
    unsafe {
        let d = rsvg_handle_get_defs(handle);
        &mut *(d as *mut Defs)
    }
}

pub fn resolve_uri(handle: *const RsvgHandle, uri: &str) -> Option<String> {
    unsafe {
        let resolved = rsvg_handle_resolve_uri(handle, uri.to_glib_none().0);
        if resolved.is_null() {
            None
        } else {
            Some(from_glib_full(resolved))
        }
    }
}

pub fn load_extern(handle: *const RsvgHandle, uri: &str) -> *const RsvgHandle {
    unsafe { rsvg_handle_load_extern(handle, uri.to_glib_none().0) }
}

pub fn get_css_styles<'a>(handle: *const RsvgHandle) -> &'a CssStyles {
    unsafe { &*(rsvg_handle_get_css_styles(handle) as *const CssStyles) }
}

pub fn get_css_styles_mut<'a>(handle: *const RsvgHandle) -> &'a mut CssStyles {
    unsafe { &mut *(rsvg_handle_get_css_styles(handle) as *mut CssStyles) }
}

pub struct BinaryData {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn acquire_data(handle: *mut RsvgHandle, url: &str) -> Result<BinaryData, glib::Error> {
    unsafe {
        let mut content_type: *mut libc::c_char = ptr::null_mut();
        let mut len = 0;
        let mut error = ptr::null_mut();

        let buf = _rsvg_handle_acquire_data(
            handle,
            url.to_glib_none().0,
            &mut content_type as *mut *mut _,
            &mut len,
            &mut error,
        );

        if buf.is_null() {
            Err(from_glib_full(error))
        } else {
            Ok(BinaryData {
                data: FromGlibContainer::from_glib_full_num(buf as *mut u8, len),
                content_type: from_glib_full(content_type),
            })
        }
    }
}

pub fn load_xml_xinclude(handle: *mut RsvgHandle, url: &str) -> bool {
    unsafe { from_glib(rsvg_load_handle_xml_xinclude(handle, url.to_glib_none().0)) }
}
