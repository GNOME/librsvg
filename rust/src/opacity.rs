/// Struct to represent an inheritable opacity property
/// https://www.w3.org/TR/SVG/masking.html#OpacityProperty

use ::cssparser::{Parser, Token, NumericValue};

use std::str::FromStr;

use parsers::ParseError;
use error::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Opacity {
    Inherit,
    Specified (f64)
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
}
