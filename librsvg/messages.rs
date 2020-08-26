use glib::translate::*;
use glib_sys::{g_log_structured_array, GLogField, G_LOG_LEVEL_CRITICAL, G_LOG_LEVEL_WARNING};

/*
  G_LOG_LEVEL_CRITICAL          = 1 << 3,
  G_LOG_LEVEL_WARNING           = 1 << 4,

#define g_critical(...) g_log_structured_standard (G_LOG_DOMAIN, G_LOG_LEVEL_CRITICAL, \
                                                   __FILE__, G_STRINGIFY (__LINE__), \
                                                   G_STRFUNC, __VA_ARGS__)
#define g_warning(...)  g_log_structured_standard (G_LOG_DOMAIN, G_LOG_LEVEL_WARNING, \
                                                   __FILE__, G_STRINGIFY (__LINE__), \
                                                   G_STRFUNC, __VA_ARGS__)
  GLogField fields[] =
    {
      { "PRIORITY", log_level_to_priority (log_level), -1 },
      { "CODE_FILE", file, -1 },
      { "CODE_LINE", line, -1 },
      { "CODE_FUNC", func, -1 },
      /* Filled in later: */
      { "MESSAGE", NULL, -1 },
      /* If @log_domain is %NULL, we will not pass this field: */
      { "GLIB_DOMAIN", log_domain, -1 },
    };

  g_log_structured_array (log_level, fields, n_fields);
 */

/// Helper for `rsvg_g_warning` and `rsvg_g_critical`
///
/// This simulates what in C would be a call to the g_warning() or g_critical()
/// macros, but with the underlying function g_log_structured_array().
///
/// If the implementation of g_warning() or g_critical() changes, we'll have
/// to change this function.
fn rsvg_g_log(level: glib_sys::GLogLevelFlags, msg: &str) {
    // stolen from gmessages.c:log_level_to_priority()
    let priority = match level {
        G_LOG_LEVEL_WARNING | G_LOG_LEVEL_CRITICAL => b"4\0",
        _ => unreachable!("please add another log level priority to rsvg_g_log()"),
    };

    let c_msg = msg.to_glib_none();
    let c_char_msg: *const libc::c_char = c_msg.0;

    // Glib's g_log_structured_standard() adds a few more fields for the source
    // file, line number, etc., but those are not terribly useful without a stack
    // trace.  So, we'll omit them.
    let fields = [
        GLogField {
            key: b"PRIORITY\0" as *const u8 as *const _,
            value: priority as *const u8 as *const _,
            length: -1,
        },
        GLogField {
            key: b"MESSAGE\0" as *const u8 as *const _,
            value: c_char_msg as *const _,
            length: msg.len() as _,
        },
        // This is the G_LOG_DOMAIN set from the Makefile
        GLogField {
            key: b"GLIB_DOMAIN\0" as *const u8 as *const _,
            value: b"librsvg\0" as *const u8 as *const _,
            length: -1,
        },
    ];

    unsafe {
        g_log_structured_array(level, fields.as_ptr(), fields.len());
    }
}

pub fn rsvg_g_warning(msg: &str) {
    rsvg_g_log(glib_sys::G_LOG_LEVEL_WARNING, msg);
}

pub fn rsvg_g_critical(msg: &str) {
    rsvg_g_log(glib_sys::G_LOG_LEVEL_CRITICAL, msg);
}

// Once Rust has a function! macro that gives us the current function name, we
// can remove the $func_name argument.
#[macro_export]
macro_rules! rsvg_return_if_fail {
    {
        $func_name:ident;
        $($condition:expr,)+
    } => {
        $(
            if !$condition {
                glib_sys::g_return_if_fail_warning(
                    b"librsvg\0" as *const u8 as *const _,
                    concat!(stringify!($func_name), "\0").as_ptr() as *const _,
                    concat!(stringify!($condition), "\0").as_ptr() as *const _,
                );
                return;
            }
        )+
    }
}

#[macro_export]
macro_rules! rsvg_return_val_if_fail {
    {
        $func_name:ident => $retval:expr;
        $($condition:expr,)+
    } => {
        $(
            if !$condition {
                glib_sys::g_return_if_fail_warning(
                    b"librsvg\0" as *const u8 as *const _,
                    concat!(stringify!($func_name), "\0").as_ptr() as *const _,
                    concat!(stringify!($condition), "\0").as_ptr() as *const _,
                );
                return $retval;
            }
        )+
    }
}
