//! Type for values in the [0.0, 1.0] range.

use cssparser::Parser;

use crate::error::*;
use crate::parsers::Parse;
use crate::util;

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd)]
pub struct UnitInterval(pub f64);

impl UnitInterval {
    pub fn clamp(val: f64) -> UnitInterval {
        UnitInterval(util::clamp(val, 0.0, 1.0))
    }
}

impl Parse for UnitInterval {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<UnitInterval, ParseError<'i>> {
        let x = f64::parse(parser)?;
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
        assert!(UnitInterval::parse_str("").is_err());
        assert!(UnitInterval::parse_str("foo").is_err());
        assert!(UnitInterval::parse_str("-x").is_err());
        assert!(UnitInterval::parse_str("0.0foo").is_err());
    }

    #[test]
    fn convert() {
        assert_eq!(u8::from(UnitInterval(0.0)), 0);
        assert_eq!(u8::from(UnitInterval(0.5)), 128);
        assert_eq!(u8::from(UnitInterval(1.0)), 255);
    }
}
