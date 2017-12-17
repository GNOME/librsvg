use ::cssparser;
use ::libc;

use ::glib::translate::*;

use parsers::Parse;
use parsers::ParseError;
use error::*;

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
    ParseError
}

// Keep this in sync with rsvg-css.h:RsvgCssColorSpec
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorSpec {
    kind: ColorKind,
    argb: u32
}

// Keep in sync with rsvg-css.h:AllowInherit
#[repr(C)]
#[derive(PartialEq, Debug)]
pub enum AllowInherit {
    No,
    Yes
}

// Keep in sync with rsvg-css.h:AllowCurrentColor
#[repr(C)]
#[derive(PartialEq, Debug)]
pub enum AllowCurrentColor {
    No,
    Yes
}

// This is the Rust version of the above
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Color {
    Inherit,
    CurrentColor,
    RGBA (cssparser::RGBA)
}

impl Parse for Color {
    type Data = (AllowInherit, AllowCurrentColor);
    type Err = AttributeError;

    fn parse (s: &str, (allow_inherit, allow_current_color): (AllowInherit, AllowCurrentColor)) -> Result<Color, AttributeError> {
        if s == "inherit" {
            if allow_inherit == AllowInherit::Yes {
                Ok (Color::Inherit)
            } else {
                Err (AttributeError::Value ("inherit is not allowed here".to_string ()))
            }
        } else {
            let mut input = cssparser::ParserInput::new (s);
            match cssparser::Color::parse (&mut cssparser::Parser::new (&mut input)) {
                Ok (cssparser::Color::CurrentColor) => {
                    if allow_current_color == AllowCurrentColor::Yes {
                        Ok (Color::CurrentColor)
                    } else {
                        Err (AttributeError::Value ("currentColor is not allowed here".to_string ()))
                    }
                },

                Ok (csscolor) => Ok (Color::from (csscolor)),

                _ => Err (AttributeError::Parse (ParseError::new ("invalid syntax for color")))
            }
        }
    }

}

impl Color {
    pub fn from_color_spec (spec: &ColorSpec) -> Result <Color, AttributeError> {
        match *spec {
            ColorSpec { kind: ColorKind::Inherit, .. }      => Ok (Color::Inherit),

            ColorSpec { kind: ColorKind::CurrentColor, .. } => Ok (Color::CurrentColor),

            ColorSpec { kind: ColorKind::ARGB, argb } => Ok (Color::RGBA (rgba_from_argb (argb))),

            ColorSpec { kind: ColorKind::ParseError, .. } => Err (AttributeError::Parse (ParseError::new ("parse error")))
        }
    }
}

fn rgba_from_argb (argb: u32) -> cssparser::RGBA {
    cssparser::RGBA::new (((argb & 0x00ff0000) >> 16) as u8,
                          ((argb & 0x0000ff00) >> 8) as u8,
                          ((argb & 0x000000ff) as u8),
                          ((argb & 0xff000000) >> 24) as u8)
}

impl From<cssparser::Color> for Color {
    fn from (c: cssparser::Color) -> Color {
        match c {
            cssparser::Color::CurrentColor => Color::CurrentColor,
            cssparser::Color::RGBA (rgba) => Color::RGBA (rgba)
        }
    }
}

impl From<u32> for Color {
    fn from (argb: u32) -> Color {
        Color::RGBA (rgba_from_argb (argb))
    }
}

impl From<Result<Color, AttributeError>> for ColorSpec {
    fn from (result: Result<Color, AttributeError>) -> ColorSpec {
        match result {
            Ok (Color::Inherit) =>
                ColorSpec {
                    kind: ColorKind::Inherit,
                    argb: 0
                },

            Ok (Color::CurrentColor) =>
                ColorSpec {
                    kind: ColorKind::CurrentColor,
                    argb: 0
                },

            Ok (Color::RGBA (rgba)) =>
                ColorSpec {
                    kind: ColorKind::ARGB,
                    argb: (u32::from(rgba.alpha) << 24 |
                           u32::from(rgba.red)   << 16 |
                           u32::from(rgba.green) << 8  |
                           u32::from(rgba.blue))
                },

            _ =>
                ColorSpec {
                    kind: ColorKind::ParseError,
                    argb: 0
                }
        }
    }
}

#[no_mangle]
pub extern fn rsvg_css_parse_color (string: *const libc::c_char,
                                    allow_inherit: AllowInherit,
                                    allow_current_color: AllowCurrentColor) -> ColorSpec {
    let s = unsafe { String::from_glib_none (string) };

    ColorSpec::from (Color::parse (&s, (allow_inherit, allow_current_color)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse (s: &str) -> ColorSpec {
        // ColorSpec::from (Color::parse (s, (AllowInherit::Yes, AllowCurrentColor::Yes)))
        rsvg_css_parse_color (s.to_glib_none ().0, AllowInherit::Yes, AllowCurrentColor::Yes)
    }

    #[test]
    fn parses_hash_hex_colors () {
        assert_eq! (parse ("#AB10fa20"), ColorSpec { kind: ColorKind::ARGB, argb: 0x20ab10fa });
        assert_eq! (parse ("#10fa20"),   ColorSpec { kind: ColorKind::ARGB, argb: 0xff10fa20 });
        assert_eq! (parse ("#abcd"),     ColorSpec { kind: ColorKind::ARGB, argb: 0xddaabbcc });
        assert_eq! (parse ("#123"),      ColorSpec { kind: ColorKind::ARGB, argb: 0xff112233 });
    }

    #[test]
    fn parses_color_keywords () {
        assert_eq! (parse ("red"),  ColorSpec { kind: ColorKind::ARGB, argb: 0xffff0000 });
        assert_eq! (parse ("lime"), ColorSpec { kind: ColorKind::ARGB, argb: 0xff00ff00 });
        assert_eq! (parse ("blue"), ColorSpec { kind: ColorKind::ARGB, argb: 0xff0000ff });
    }

    #[test]
    fn parses_color_functions () {
        assert_eq! (parse ("rgb(255, 0, 0)"), ColorSpec { kind: ColorKind::ARGB, argb: 0xffff0000 });
        assert_eq! (parse ("rgb(0, 255, 0)"), ColorSpec { kind: ColorKind::ARGB, argb: 0xff00ff00 });
        assert_eq! (parse ("rgb(0, 0, 255)"), ColorSpec { kind: ColorKind::ARGB, argb: 0xff0000ff });
    }

    #[test]
    fn parses_current_color () {
        assert_eq! (parse ("currentColor"), ColorSpec { kind: ColorKind::CurrentColor, argb: 0 });
    }

    fn make_error () -> ColorSpec {
        ColorSpec {
            kind: ColorKind::ParseError,
            argb: 0
        }
    }

    #[test]
    fn invalid_hash_hex_colors_yield_error () {
        assert_eq! (parse ("#"), make_error ());
        assert_eq! (parse ("#xyz"), make_error ());
        assert_eq! (parse ("#112233gg"), make_error ());
    }

    #[test]
    fn invalid_colors_yield_error () {
        assert_eq! (parse (""), make_error ());
        assert_eq! (parse ("foo"), make_error ());
        assert_eq! (parse ("rgb(chilaquil)"), make_error ());
        assert_eq! (parse ("rgb(1, 2, 3, 4, 5)"), make_error ());
    }

    #[test]
    fn yields_error_on_disallowed_current_color () {
        assert_eq! (ColorSpec::from (Color::parse ("currentColor", (AllowInherit::Yes, AllowCurrentColor::No))),
                    make_error ());
    }

    #[test]
    fn yields_error_on_disallowed_inherit () {
        assert_eq! (ColorSpec::from (Color::parse ("inherit", (AllowInherit::No, AllowCurrentColor::Yes))),
                    make_error ());
    }

    fn test_roundtrip (s: &str) {
        let result = Color::parse (s, (AllowInherit::Yes, AllowCurrentColor::Yes));
        let result2 = result.clone ();
        let spec = ColorSpec::from (result2);

        if result.is_ok () {
            assert_eq! (Color::from_color_spec (&spec), result);
        } else {
            assert! (Color::from_color_spec (&spec).is_err ());
        }
    }

    #[test]
    fn roundtrips () {
        test_roundtrip ("inherit");
        test_roundtrip ("currentColor");
        test_roundtrip ("#aabbccdd");
        test_roundtrip ("papadzul");
    }

    #[test]
    fn from_argb () {
        assert_eq! (Color::from (0xaabbccdd),
                    Color::RGBA (cssparser::RGBA::new (0xbb, 0xcc, 0xdd, 0xaa)));
    }
}
