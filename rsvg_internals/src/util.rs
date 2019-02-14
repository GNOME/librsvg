use libc;

use std::ffi::CStr;
use std::str;

#[cfg(feature = "c-library")]
use glib::translate::*;

/// Converts a `char *` which is known to be valid UTF-8 into a `&str`
///
/// The usual `from_glib_none(s)` allocates an owned String.  The
/// purpose of `utf8_cstr()` is to get a temporary string slice into a
/// C string which is already known to be valid UTF-8; for example,
/// as for strings which come from `libxml2`.
pub unsafe fn utf8_cstr<'a>(s: *const libc::c_char) -> &'a str {
    assert!(!s.is_null());

    str::from_utf8_unchecked(CStr::from_ptr(s).to_bytes())
}

pub fn clamp<T: PartialOrd>(val: T, low: T, high: T) -> T {
    if val < low {
        low
    } else if val > high {
        high
    } else {
        val
    }
}

#[cfg(feature = "c-library")]
pub fn rsvg_g_warning(msg: &str) {
    unsafe {
        extern "C" {
            fn rsvg_g_warning_from_c(msg: *const libc::c_char);
        }

        rsvg_g_warning_from_c(msg.to_glib_none().0);
    }
}

#[cfg(not(feature = "c-library"))]
pub fn rsvg_g_warning(_msg: &str) {
    // The only callers of this are in handle.rs. When those functions
    // are called from the Rust API, they are able to return a
    // meaningful error code, but the C API isn't - so they issues a
    // g_warning() instead.
}
