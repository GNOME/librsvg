#[macro_export]
macro_rules! rsvg_log {
    (
        $($arg:tt)+
    ) => {
        if ::log::log_enabled() {
            println!("{}", format_args!($($arg)+));
        }
    };
}

pub fn log_enabled() -> bool {
    lazy_static! {
        static ref ENABLED: bool = ::std::env::var_os("RSVG_LOG").is_some();
    }

    *ENABLED
}
