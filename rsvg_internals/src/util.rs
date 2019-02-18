use libc;

use std::ffi::CStr;
use std::borrow::Cow;
use std::str;

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

extern "C" {
    fn rsvg_g_warning_from_c(msg: *const libc::c_char);
}

pub fn rsvg_g_warning(msg: &str) {
    unsafe {
        rsvg_g_warning_from_c(msg.to_glib_none().0);
    }
}
