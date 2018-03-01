/// Struct to represent an inheritable opacity property
/// <https://www.w3.org/TR/SVG/masking.html#OpacityProperty>
use cssparser::{Parser, ParserInput, Token};
use libc;

use std::str::FromStr;

use error::*;
use parsers::ParseError;
use util::utf8_cstr;

// Keep this in sync with rsvg-css.h:RsvgOpacityKind
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OpacityKind {
    Inherit,
    Specified,
    ParseError,
}

// Keep this in sync with rsvg-css.h:RsvgOpacitySpec
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OpacitySpec {
    kind: OpacityKind,
    opacity: u8,
}

// This is the Rust version of the above
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Opacity {
    Inherit,
    Specified(f64),
}

impl From<Result<Opacity, AttributeError>> for OpacitySpec {
    fn from(result: Result<Opacity, AttributeError>) -> OpacitySpec {
        match result {
            Ok(Opacity::Inherit) => OpacitySpec {
                kind: OpacityKind::Inherit,
                opacity: 0,
            },

            Ok(Opacity::Specified(val)) => OpacitySpec {
                kind: OpacityKind::Specified,
                opacity: opacity_to_u8(val),
            },

            _ => OpacitySpec {
                kind: OpacityKind::ParseError,
                opacity: 0,
            },
        }
    }
}

pub fn opacity_to_u8(val: f64) -> u8 {
    (val * 255.0 + 0.5).floor() as u8
}

fn make_err() -> AttributeError {
    AttributeError::Parse(ParseError::new("expected 'inherit' or number"))
}

impl FromStr for Opacity {
    type Err = AttributeError;

    fn from_str(s: &str) -> Result<Opacity, AttributeError> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        let opacity = {
            let token = parser.next().map_err(|_| make_err())?;

            match *token {
                Token::Ident(ref cow) => {
                    if cow.as_ref() == "inherit" {
                        Opacity::Inherit
                    } else {
                        return Err(make_err());
                    }
                }

                Token::Number { value, .. } => {
                    if value < 0.0 {
                        Opacity::Specified(0.0)
                    } else if value > 1.0 {
                        Opacity::Specified(1.0)
                    } else {
                        Opacity::Specified(f64::from(value))
                    }
                }

                _ => return Err(make_err()),
            }
        };

        parser.expect_exhausted().map_err(|_| make_err())?;

        Ok(opacity)
    }
}

impl Opacity {
    pub fn from_opacity_spec(spec: &OpacitySpec) -> Result<Opacity, AttributeError> {
        match *spec {
            OpacitySpec {
                kind: OpacityKind::Inherit,
                ..
            } => Ok(Opacity::Inherit),

            OpacitySpec {
                kind: OpacityKind::Specified,
                opacity,
            } => Ok(Opacity::Specified(f64::from(opacity) / 255.0)),

            OpacitySpec {
                kind: OpacityKind::ParseError,
                ..
            } => Err(AttributeError::Parse(ParseError::new("parse error"))),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_css_parse_opacity(string: *const libc::c_char) -> OpacitySpec {
    let s = unsafe { utf8_cstr(string) };

    OpacitySpec::from(Opacity::from_str(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glib::translate::*;
    use std::str::FromStr;

    #[test]
    fn parses_inherit() {
        assert_eq!(Opacity::from_str("inherit"), Ok(Opacity::Inherit));
    }

    #[test]
    fn parses_number() {
        assert_eq!(Opacity::from_str("0"), Ok(Opacity::Specified(0.0)));
        assert_eq!(Opacity::from_str("1"), Ok(Opacity::Specified(1.0)));
        assert_eq!(Opacity::from_str("0.5"), Ok(Opacity::Specified(0.5)));
    }

    #[test]
    fn parses_out_of_range_number() {
        assert_eq!(Opacity::from_str("-10"), Ok(Opacity::Specified(0.0)));
        assert_eq!(Opacity::from_str("10"), Ok(Opacity::Specified(1.0)));
    }

    #[test]
    fn errors_on_invalid_input() {
        assert!(is_parse_error(&Opacity::from_str("")));
        assert!(is_parse_error(&Opacity::from_str("foo")));
        assert!(is_parse_error(&Opacity::from_str("-x")));
    }

    #[test]
    fn errors_on_extra_input() {
        assert!(is_parse_error(&Opacity::from_str(
            "inherit a million dollars"
        )));
        assert!(is_parse_error(&Opacity::from_str("0.0foo")));
    }

    fn parse(s: &str) -> OpacitySpec {
        rsvg_css_parse_opacity(s.to_glib_none().0)
    }

    #[test]
    fn converts_result_to_opacity_spec() {
        assert_eq!(
            parse("inherit"),
            OpacitySpec {
                kind: OpacityKind::Inherit,
                opacity: 0,
            }
        );

        assert_eq!(
            parse("0"),
            OpacitySpec {
                kind: OpacityKind::Specified,
                opacity: 0,
            }
        );
        assert_eq!(
            parse("1"),
            OpacitySpec {
                kind: OpacityKind::Specified,
                opacity: 255,
            }
        );

        assert_eq!(
            parse("foo"),
            OpacitySpec {
                kind: OpacityKind::ParseError,
                opacity: 0,
            }
        );
    }

    fn test_roundtrip(s: &str) {
        let result = Opacity::from_str(s);
        let result2 = result.clone();
        let spec = OpacitySpec::from(result2);

        if result.is_ok() {
            assert_eq!(Opacity::from_opacity_spec(&spec), result);
        } else {
            assert!(Opacity::from_opacity_spec(&spec).is_err());
        }
    }

    #[test]
    fn roundtrips() {
        test_roundtrip("inherit");
        test_roundtrip("0");
        test_roundtrip("1.0");
        test_roundtrip("chilaquil");
    }
}
