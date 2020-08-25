use std::ffi::CStr;

use rsvg_internals::{Color, Parse};

// There are two quirks here:
//
// First, we need to expose the Color algebraic type *and* a parse
// error to C, but we can't repr(C) them plainly.  So, we define a
// ColorKind enum and a ColorSpec struct that can both be represented
// in C.
//
// Second, the C code in librsvg expects ARGB colors passed around as
// guint32.  However, in Rust we'd prefer to use cssparser's RGBA
// structure, which has explicit fields for red/green/blue/alpha.
// We'll do those conversions here, for the benefit of the C code, and
// then just wait until the C code gradually disappears.

// Keep this in sync with rsvg-css.h:RsvgCssColorKind
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorKind {
    ARGB,
    ParseError,
}

// Keep this in sync with rsvg-css.h:RsvgCssColorSpec
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorSpec {
    kind: ColorKind,
    argb: u32,
}

fn rgba_to_argb(r: u8, g: u8, b: u8, a: u8) -> u32 {
    u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_css_parse_color(string: *const libc::c_char) -> ColorSpec {
    let s = CStr::from_ptr(string).to_string_lossy();
    let r = <Color as Parse>::parse_str(&s);

    match r {
        Ok(Color::RGBA(rgba)) => ColorSpec {
            kind: ColorKind::ARGB,
            argb: rgba_to_argb(rgba.red, rgba.green, rgba.blue, rgba.alpha),
        },

        _ => ColorSpec {
            kind: ColorKind::ParseError,
            argb: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glib::translate::*;

    fn parse(s: &str) -> ColorSpec {
        unsafe { rsvg_css_parse_color(s.to_glib_none().0) }
    }

    #[test]
    fn parses_hash_hex_colors() {
        assert_eq!(
            parse("#AB10fa20"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0x20ab10fa,
            }
        );
        assert_eq!(
            parse("#10fa20"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xff10fa20,
            }
        );
        assert_eq!(
            parse("#abcd"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xddaabbcc,
            }
        );
        assert_eq!(
            parse("#123"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xff112233,
            }
        );
    }

    #[test]
    fn parses_color_keywords() {
        assert_eq!(
            parse("red"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xffff0000,
            }
        );
        assert_eq!(
            parse("lime"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xff00ff00,
            }
        );
        assert_eq!(
            parse("blue"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xff0000ff,
            }
        );
    }

    #[test]
    fn parses_color_functions() {
        assert_eq!(
            parse("rgb(255, 0, 0)"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xffff0000,
            }
        );
        assert_eq!(
            parse("rgb(0, 255, 0)"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xff00ff00,
            }
        );
        assert_eq!(
            parse("rgb(0, 0, 255)"),
            ColorSpec {
                kind: ColorKind::ARGB,
                argb: 0xff0000ff,
            }
        );
    }

    #[test]
    fn current_color_is_error() {
        assert_eq!(
            parse("currentColor"),
            ColorSpec {
                kind: ColorKind::ParseError,
                argb: 0,
            }
        );
    }

    fn make_error() -> ColorSpec {
        ColorSpec {
            kind: ColorKind::ParseError,
            argb: 0,
        }
    }

    #[test]
    fn invalid_hash_hex_colors_yield_error() {
        assert_eq!(parse("#"), make_error());
        assert_eq!(parse("#xyz"), make_error());
        assert_eq!(parse("#112233gg"), make_error());
    }

    #[test]
    fn invalid_colors_yield_error() {
        assert_eq!(parse(""), make_error());
        assert_eq!(parse("foo"), make_error());
        assert_eq!(parse("rgb(chilaquil)"), make_error());
        assert_eq!(parse("rgb(1, 2, 3, 4, 5)"), make_error());
    }
}
