use glib::translate::*;
use libc;

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
}

pub fn get_defs<'a>(handle: *const RsvgHandle) -> &'a Defs {
    unsafe {
        let d = rsvg_handle_get_defs(handle);
        &*(d as *const Defs)
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
