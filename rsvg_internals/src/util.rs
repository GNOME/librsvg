use libc;

use std::borrow::Cow;
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

/// Error-tolerant C string import
pub unsafe fn cstr<'a>(s: *const libc::c_char) -> Cow<'a, str> {
    if s.is_null() {
        return Cow::Borrowed("(null)");
    }
    CStr::from_ptr(s).to_string_lossy()
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
    // This, and rsvg_g_critical() below, are intended to be called
    // from Rust code which is directly called from the C API.  When
    // this crate is being built as part of the c-library, the purpose
    // of the functions is to call the corresponding glib macros to
    // print a warning or a critical error message.  When this crate
    // is being built as part of the standalone Rust crate, they do
    // nothing, since the Rust code is able to propagate an error code
    // all the way to the crate's public API.
}

#[cfg(feature = "c-library")]
pub fn rsvg_g_critical(msg: &str) {
    unsafe {
        extern "C" {
            fn rsvg_g_critical_from_c(msg: *const libc::c_char);
        }

        rsvg_g_critical_from_c(msg.to_glib_none().0);
    }
}

#[cfg(not(feature = "c-library"))]
pub fn rsvg_g_critical(_msg: &str) {
}
