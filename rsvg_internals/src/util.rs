use libc;

use std::ffi::CStr;
use std::str;

// In paint servers (patterns, gradients, etc.), we have an
// Option<String> for fallback names.  This is a utility function to
// clone one of those.
pub fn clone_fallback_name(fallback: &Option<String>) -> Option<String> {
    if let Some(ref fallback_name) = *fallback {
        Some(fallback_name.clone())
    } else {
        None
    }
}

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
