/// Struct to represent an inheritable opacity property
/// https://www.w3.org/TR/SVG/masking.html#OpacityProperty

use ::cssparser::{Parser, Token, NumericValue};
use ::libc;

use std::str::FromStr;

use ::glib::translate::*;

use parsers::ParseError;
use error::*;

// Keep this in sync with rsvg-css.h:RsvgOpacityKind
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OpacityKind {
    Inherit,
    Specified,
    ParseError
}

// Keep this in sync with rsvg-css.h:RsvgOpacitySpec
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OpacitySpec {
    kind: OpacityKind,
    opacity: u8
}

// This is the Rust version of the above
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Opacity {
    Inherit,
    Specified (f64)
}

impl From<Result<Opacity, AttributeError>> for OpacitySpec {
    fn from (result: Result<Opacity, AttributeError>) -> OpacitySpec {
        match result {
            Ok (Opacity::Inherit) =>
                OpacitySpec {
                    kind: OpacityKind::Inherit,
                    opacity: 0
                },

            Ok (Opacity::Specified (val)) =>
                OpacitySpec {
                    kind: OpacityKind::Specified,
                    opacity: opacity_to_u8 (val)
                },

            _ =>
                OpacitySpec {
                    kind: OpacityKind::ParseError,
                    opacity: 0
                }
        }
    }
}

fn opacity_to_u8 (val: f64) -> u8 {
    (val * 255.0 + 0.5).floor () as u8
}

impl FromStr for Opacity {
    type Err = AttributeError;

    fn from_str (s: &str) -> Result<Opacity, AttributeError> {
        let mut parser = Parser::new (s);

        let token = parser.next ();
        let result = match token {
            Ok (Token::Ident (value)) => {
                if value == "inherit" {
                    Ok (Opacity::Inherit)
                } else {
                    Err (())
                }
            },

            Ok (Token::Number (NumericValue { value, .. })) => {
                if value < 0.0 {
                    Ok (Opacity::Specified (0.0))
                } else if value > 1.0 {
                    Ok (Opacity::Specified (1.0))
                } else {
                    Ok (Opacity::Specified (value as f64))
                }
            },

            _ => Err (())
        };

        result.and_then (|opacity|
                         parser.expect_exhausted ()
                         .map (|_| opacity)
                         .map_err (|_| ()))
            .map_err (|_| AttributeError::Parse (ParseError::new ("expected 'inherit' or number")))
    }
}

#[no_mangle]
pub extern fn rsvg_css_parse_opacity (string: *const libc::c_char) -> OpacitySpec {
    let s = unsafe { String::from_glib_none (string) };

    OpacitySpec::from (Opacity::from_str (&s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_inherit () {
        assert_eq! (Opacity::from_str ("inherit"), Ok (Opacity::Inherit));
    }

    #[test]
    fn parses_number () {
        assert_eq! (Opacity::from_str ("0"),   Ok (Opacity::Specified (0.0)));
        assert_eq! (Opacity::from_str ("1"),   Ok (Opacity::Specified (1.0)));
        assert_eq! (Opacity::from_str ("0.5"), Ok (Opacity::Specified (0.5)));
    }

    #[test]
    fn parses_out_of_range_number () {
        assert_eq! (Opacity::from_str ("-10"), Ok (Opacity::Specified (0.0)));
        assert_eq! (Opacity::from_str ("10"),  Ok (Opacity::Specified (1.0)));
    }

    #[test]
    fn errors_on_invalid_input () {
        assert! (is_parse_error (&Opacity::from_str ("")));
        assert! (is_parse_error (&Opacity::from_str ("foo")));
        assert! (is_parse_error (&Opacity::from_str ("-x")));
    }

    #[test]
    fn errors_on_extra_input () {
        assert! (is_parse_error (&Opacity::from_str ("inherit a million dollars")));
        assert! (is_parse_error (&Opacity::from_str ("0.0foo")));
    }

    fn parse (s: &str) -> OpacitySpec {
        rsvg_css_parse_opacity (s.to_glib_none ().0)
    }

    #[test]
    fn converts_result_to_opacity_spec () {
        assert_eq! (parse ("inherit"),
                    OpacitySpec { kind: OpacityKind::Inherit,
                                  opacity: 0 });

        assert_eq! (parse ("0"),
                    OpacitySpec { kind: OpacityKind::Specified,
                                  opacity: 0 });
        assert_eq! (parse ("1"),
                    OpacitySpec { kind: OpacityKind::Specified,
                                  opacity: 255 });

        assert_eq! (parse ("foo"),
                    OpacitySpec { kind: OpacityKind::ParseError,
                                  opacity: 0 });
    }
}
