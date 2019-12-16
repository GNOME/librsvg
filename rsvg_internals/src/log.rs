//! Utilities for logging messages from the library.

use once_cell::sync::Lazy;

#[macro_export]
macro_rules! rsvg_log {
    (
        $($arg:tt)+
    ) => {
        if $crate::log::log_enabled() {
            println!("{}", format_args!($($arg)+));
        }
    };
}

pub fn log_enabled() -> bool {
    static ENABLED: Lazy<bool> = Lazy::new(|| ::std::env::var_os("RSVG_LOG").is_some());

    *ENABLED
}
