//! Miscellaneous utilities.

use std::borrow::Cow;
use std::ffi::CStr;
use std::mem::transmute;
use std::str;

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

/// Casts a pointer to `c_char` to a pointer to `u8`.
///
/// The obvious `p as *const u8` or `p as *const _` produces a
/// trivial_casts warning when compiled on aarch64, where `c_char` is
/// unsigned (on Intel, it is signed, so the cast works).
///
/// We do this here with a `transmute`, which is awkward to type,
/// so wrap it in a function.
pub unsafe fn c_char_as_u8_ptr(p: *const libc::c_char) -> *const u8 {
    transmute::<_, *const u8>(p)
}

/// Casts a pointer to `c_char` to a pointer to mutable `u8`.
///
/// See [`c_char_as_u8_ptr`] for the reason for this.
pub unsafe fn c_char_as_u8_ptr_mut(p: *mut libc::c_char) -> *mut u8 {
    transmute::<_, *mut u8>(p)
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
