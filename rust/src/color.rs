use ::cssparser;

// Keep this in sync with rsvg-css.h:RsvgCssColorKind
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorKind {
    Inherit,
    CurrentColor,
    ARGB,
    ParseError
}

// Keep this in sync with rsvg-css.h:RsvgCssColorSpec
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColorSpec {
    kind: ColorKind,
    argb: u32
}

impl<'i> From<Result<cssparser::Color, cssparser::BasicParseError<'i>>> for ColorSpec {
    fn from (result: Result<cssparser::Color, cssparser::BasicParseError<'i>>) -> ColorSpec {
        match result {
            Ok (cssparser::Color::CurrentColor) =>
                ColorSpec {
                    kind: ColorKind::CurrentColor,
                    argb: 0
                },

            Ok (cssparser::Color::RGBA (rgba)) =>
                ColorSpec {
                    kind: ColorKind::ARGB,
                    argb: ((rgba.alpha as u32) << 24 |
                           (rgba.red as u32)   << 16 |
                           (rgba.green as u32) << 8  |
                           (rgba.blue as u32))
                },

            _ =>
                ColorSpec {
                    kind: ColorKind::ParseError,
                    argb: 0
                }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn parse (s: &str) -> ColorSpec {
        ColorSpec::from (cssparser::Color::parse (&mut cssparser::Parser::new (s)))
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
    }
}
