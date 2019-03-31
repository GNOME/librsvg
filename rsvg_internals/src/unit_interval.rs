use cssparser::Parser;

use crate::error::*;
use crate::parsers::{CssParserExt, Parse, ParseError};
use crate::util;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct UnitInterval(pub f64);

impl UnitInterval {
    pub fn clamp(val: f64) -> UnitInterval {
        UnitInterval(util::clamp(val, 0.0, 1.0))
    }
}

impl Parse for UnitInterval {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<UnitInterval, ValueErrorKind> {
        let x = f64::from(
            parser
                .expect_finite_number()
                .map_err(|_| ValueErrorKind::Parse(ParseError::new("expected number")))?,
        );

        Ok(UnitInterval::clamp(x))
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

    #[test]
    fn clamps() {
        assert_eq!(UnitInterval::clamp(-1.0), UnitInterval(0.0));
        assert_eq!(UnitInterval::clamp(0.0), UnitInterval(0.0));
        assert_eq!(UnitInterval::clamp(0.5), UnitInterval(0.5));
        assert_eq!(UnitInterval::clamp(1.0), UnitInterval(1.0));
        assert_eq!(UnitInterval::clamp(2.0), UnitInterval(1.0));
    }

    #[test]
    fn parses_number() {
        assert_eq!(UnitInterval::parse_str("0"), Ok(UnitInterval(0.0)));
        assert_eq!(UnitInterval::parse_str("1"), Ok(UnitInterval(1.0)));
        assert_eq!(UnitInterval::parse_str("0.5"), Ok(UnitInterval(0.5)));
    }

    #[test]
    fn parses_out_of_range_number() {
        assert_eq!(UnitInterval::parse_str("-10"), Ok(UnitInterval(0.0)));
        assert_eq!(UnitInterval::parse_str("10"), Ok(UnitInterval(1.0)));
    }

    #[test]
    fn errors_on_invalid_input() {
        assert!(is_parse_error(&UnitInterval::parse_str("")));
        assert!(is_parse_error(&UnitInterval::parse_str("foo")));
        assert!(is_parse_error(&UnitInterval::parse_str("-x")));
        assert!(is_parse_error(&UnitInterval::parse_str("0.0foo")));
    }

    #[test]
    fn convert() {
        assert_eq!(u8::from(UnitInterval(0.0)), 0);
        assert_eq!(u8::from(UnitInterval(0.5)), 128);
        assert_eq!(u8::from(UnitInterval(1.0)), 255);
    }
}
