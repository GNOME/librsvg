use cssparser;
use libc;

use error::*;
use parsers::Parse;
use parsers::ParseError;
use util::utf8_cstr;

pub use cssparser::Color;

impl Parse for cssparser::Color {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: Self::Data) -> Result<cssparser::Color, AttributeError> {
        let mut input = cssparser::ParserInput::new(s);
        cssparser::Color::parse(&mut cssparser::Parser::new(&mut input))
            .map_err(|_| AttributeError::Parse(ParseError::new("invalid syntax for color")))
    }
}

impl Parse for cssparser::RGBA {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: Self::Data) -> Result<cssparser::RGBA, AttributeError> {
        let mut input = cssparser::ParserInput::new(s);
        match cssparser::Color::parse(&mut cssparser::Parser::new(&mut input)) {
            Ok(cssparser::Color::RGBA(rgba)) => Ok(rgba),
            Ok(cssparser::Color::CurrentColor) => Err(AttributeError::Value(
                "currentColor is not allowed here".to_string(),
            )),
            _ => Err(AttributeError::Parse(ParseError::new(
                "invalid syntax for color",
            ))),
        }
    }
}

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
    Inherit,
    CurrentColor,
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

pub fn from_color_spec(spec: &ColorSpec) -> Result<Option<cssparser::Color>, AttributeError> {
    match *spec {
        ColorSpec {
            kind: ColorKind::Inherit,
            ..
        } => Ok(None),

        ColorSpec {
            kind: ColorKind::CurrentColor,
            ..
        } => Ok(Some(cssparser::Color::CurrentColor)),

        ColorSpec {
            kind: ColorKind::ARGB,
            argb,
        } => Ok(Some(cssparser::Color::RGBA(rgba_from_argb(argb)))),

        ColorSpec {
            kind: ColorKind::ParseError,
            ..
        } => Err(AttributeError::Parse(ParseError::new("parse error"))),
    }
}

pub fn rgba_from_argb(argb: u32) -> cssparser::RGBA {
    cssparser::RGBA::new(
        ((argb & 0x00ff_0000) >> 16) as u8,
        ((argb & 0x0000_ff00) >> 8) as u8,
        (argb & 0x0000_00ff) as u8,
        ((argb & 0xff00_0000) >> 24) as u8,
    )
}

pub fn rgba_to_argb(rgba: cssparser::RGBA) -> u32 {
    u32::from(rgba.alpha) << 24 | u32::from(rgba.red) << 16 | u32::from(rgba.green) << 8
        | u32::from(rgba.blue)
}

impl From<Result<Option<cssparser::Color>, AttributeError>> for ColorSpec {
    fn from(result: Result<Option<cssparser::Color>, AttributeError>) -> ColorSpec {
        match result {
            Ok(None) => ColorSpec {
                kind: ColorKind::Inherit,
                argb: 0,
            },

            Ok(Some(cssparser::Color::CurrentColor)) => ColorSpec {
                kind: ColorKind::CurrentColor,
                argb: 0,
            },

            Ok(Some(cssparser::Color::RGBA(rgba))) => ColorSpec {
                kind: ColorKind::ARGB,
                argb: rgba_to_argb(rgba),
            },

            _ => ColorSpec {
                kind: ColorKind::ParseError,
                argb: 0,
            },
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_css_parse_color(string: *const libc::c_char) -> ColorSpec {
    let s = unsafe { utf8_cstr(string) };

    if s == "inherit" {
        ColorSpec {
            kind: ColorKind::Inherit,
            argb: 0,
        }
    } else {
        ColorSpec::from(<Color as Parse>::parse(s, ()).map(|v| Some(v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glib::translate::*;

    fn parse(s: &str) -> ColorSpec {
        rsvg_css_parse_color(s.to_glib_none().0)
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
    fn parses_current_color() {
        assert_eq!(
            parse("currentColor"),
            ColorSpec {
                kind: ColorKind::CurrentColor,
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

    fn test_roundtrip(s: &str) {
        let result = <Color as Parse>::parse(s, ()).map(|v| Some(v));
        let result2 = result.clone();
        let spec = ColorSpec::from(result2);

        if result.is_ok() {
            assert_eq!(from_color_spec(&spec), result);
        } else {
            assert!(from_color_spec(&spec).is_err());
        }
    }

    #[test]
    fn roundtrips() {
        test_roundtrip("currentColor");
        test_roundtrip("#aabbccdd");
        test_roundtrip("papadzul");
    }

    #[test]
    fn from_argb() {
        assert_eq!(
            rgba_from_argb(0xaabbccdd),
            cssparser::RGBA::new(0xbb, 0xcc, 0xdd, 0xaa)
        );
    }
}
