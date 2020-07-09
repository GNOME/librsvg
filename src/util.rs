//! Miscellaneous utilities.

use std::borrow::Cow;
use std::ffi::CStr;
use std::str;

use crate::error::RenderingError;

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

pub unsafe fn opt_utf8_cstr<'a>(s: *const libc::c_char) -> Option<&'a str> {
    if s.is_null() {
        None
    } else {
        Some(utf8_cstr(s))
    }
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

#[macro_export]
macro_rules! enum_default {
    ($name:ident, $default:expr) => {
        impl Default for $name {
            #[inline]
            fn default() -> $name {
                $default
            }
        }
    };
}

/// Return an error if a Cairo context is in error.
///
/// https://github.com/gtk-rs/gtk-rs/issues/74 - with cairo-rs 0.9.0, the `Context` objet
/// lost the ability to get its error status queried.  So we do it by hand with `cairo_sys`.
pub fn check_cairo_context(cr: &cairo::Context) -> Result<(), RenderingError> {
    unsafe {
        let cr_raw = cr.to_raw_none();

        let status = cairo_sys::cairo_status(cr_raw);
        if status == cairo_sys::STATUS_SUCCESS {
            Ok(())
        } else {
            let status: cairo::Error = status.into();
            Err(RenderingError::from(status))
        }
    }
}
