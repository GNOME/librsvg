use cssparser;
use std::str::FromStr;

use error::*;
use parsers::ParseError;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UnitInterval(pub f64);

impl Default for UnitInterval {
    fn default() -> UnitInterval {
        UnitInterval(1.0)
    }
}

impl FromStr for UnitInterval {
    type Err = AttributeError;

    fn from_str(s: &str) -> Result<UnitInterval, AttributeError> {
        let mut input = cssparser::ParserInput::new(s);
        let mut parser = cssparser::Parser::new(&mut input);

        let x = f64::from(
            parser
                .expect_number()
                .map_err(|_| AttributeError::Parse(ParseError::new("expected number")))?,
        );

        let cx = if x < 0.0 {
            0.0
        } else if x > 1.0 {
            1.0
        } else {
            x
        };

        Ok(UnitInterval(cx))
    }
}

impl From<UnitInterval> for u8 {
    fn from(val: UnitInterval) -> u8 {
        let UnitInterval(x) = val;
        (x * 255.0 + 0.5).floor() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_number() {
        assert_eq!("0".parse(), Ok(UnitInterval(0.0)));
        assert_eq!("1".parse(), Ok(UnitInterval(1.0)));
        assert_eq!("0.5".parse(), Ok(UnitInterval(0.5)));
    }

    #[test]
    fn parses_out_of_range_number() {
        assert_eq!("-10".parse(), Ok(UnitInterval(0.0)));
        assert_eq!("10".parse(), Ok(UnitInterval(1.0)));
    }

    #[test]
    fn errors_on_invalid_input() {
        assert!(is_parse_error(&UnitInterval::from_str("")));
        assert!(is_parse_error(&UnitInterval::from_str("foo")));
        assert!(is_parse_error(&UnitInterval::from_str("-x")));
        assert!(is_parse_error(&UnitInterval::from_str("0.0foo")));
    }

    #[test]
    fn convert() {
        assert_eq!(u8::from(UnitInterval(0.0)), 0);
        assert_eq!(u8::from(UnitInterval(0.5)), 128);
        assert_eq!(u8::from(UnitInterval(1.0)), 255);
    }
}
