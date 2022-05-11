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

/// Captures the basic state of a [`cairo::Context`] for logging purposes.
///
/// A librsvg "transaction" like rendering a
/// [`crate::api::SvgHandle`], which takes a Cairo context, depends on the state of the
/// context as it was passed in by the caller.  For example, librsvg may decide to
/// operate differently depending on the context's target surface type, or its current
/// transformation matrix.  This struct captures that sort of information.
#[derive(Copy, Clone, Debug, PartialEq)]
struct CairoContextState {
    surface_type: cairo::SurfaceType,
}

impl CairoContextState {
    fn new(cr: &cairo::Context) -> Self {
        let surface_type = cr.target().type_();

        Self { surface_type }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_cr_surface_type() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 10, 10).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let state = CairoContextState::new(&cr);

        assert_eq!(
            CairoContextState {
                surface_type: cairo::SurfaceType::Image,
            },
            state,
        );

        let surface = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let state = CairoContextState::new(&cr);

        assert_eq!(
            CairoContextState {
                surface_type: cairo::SurfaceType::Recording,
            },
            state,
        );
    }
}
