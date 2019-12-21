//! The `Parse` trait for CSS properties, and utilities for parsers.

use cssparser::{Parser, ParserInput};
use markup5ever::QualName;

use std::str;

use crate::error::*;

/// Trait to parse values using `cssparser::Parser`.
pub trait Parse: Sized {
    /// Parses a value out of the `parser`.
    ///
    /// All value types should implement this for composability.
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>>;

    /// Convenience function to parse a value out of a `&str`.
    ///
    /// This is useful mostly for tests which want to avoid creating a
    /// `cssparser::Parser` by hand.
    fn parse_str<'i>(s: &'i str) -> Result<Self, CssParseError<'i>> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        Self::parse(&mut parser).and_then(|r| {
            // FIXME: parser.expect_exhausted()?;
            Ok(r)
        })
    }
}

/// Consumes a comma if it exists, or does nothing.
pub fn optional_comma<'i, 't>(parser: &mut Parser<'i, 't>) {
    let _ = parser.try_parse(|p| p.expect_comma());
}

pub fn finite_f32(n: f32) -> Result<f32, ValueErrorKind> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(ValueErrorKind::Value("expected finite number".to_string()))
    }
}

pub trait ParseValue<T: Parse> {
    /// Parses a `value` string into a type `T`.
    fn parse(&self, value: &str) -> Result<T, NodeError>;

    /// Parses a `value` string into a type `T` with an optional validation function.
    fn parse_and_validate<F: FnOnce(T) -> Result<T, ValueErrorKind>>(
        &self,
        value: &str,
        validate: F,
    ) -> Result<T, NodeError>;
}

impl<T: Parse> ParseValue<T> for QualName {
    fn parse(&self, value: &str) -> Result<T, NodeError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        T::parse(&mut parser).attribute(self.clone())
    }

    fn parse_and_validate<F: FnOnce(T) -> Result<T, ValueErrorKind>>(
        &self,
        value: &str,
        validate: F,
    ) -> Result<T, NodeError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        let v = T::parse(&mut parser).attribute(self.clone())?;

        validate(v)
            .map_err(|e| parser.new_custom_error(e))
            .attribute(self.clone())
    }
}

impl Parse for f64 {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>> {
        let loc = parser.current_source_location();
        parser.expect_number().map_err(|e| e.into()).and_then(|n| {
            if n.is_finite() {
                Ok(f64::from(n))
            } else {
                Err(loc.new_custom_error(ValueErrorKind::value_error("expected finite number")))
            }
        })
    }
}

/// CSS number-optional-number
///
/// https://www.w3.org/TR/SVG/types.html#DataTypeNumberOptionalNumber
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NumberOptionalNumber<T: Parse>(pub T, pub T);

impl<T: Parse + Copy> Parse for NumberOptionalNumber<T> {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>> {
        let x = Parse::parse(parser)?;

        if !parser.is_exhausted() {
            optional_comma(parser);
            let y = Parse::parse(parser)?;
            parser.expect_exhausted()?;
            Ok(NumberOptionalNumber(x, y))
        } else {
            Ok(NumberOptionalNumber(x, x))
        }
    }
}

impl Parse for i32 {
    /// CSS integer
    ///
    /// https://www.w3.org/TR/SVG11/types.html#DataTypeInteger
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>> {
        Ok(parser.expect_integer()?)
    }
}

/// Parses a list of identifiers from a `cssparser::Parser`
///
/// # Example
///
/// ```ignore
/// let my_boolean = parse_identifiers!(
///     parser,
///     "true" => true,
///     "false" => false,
/// )?;
/// ```
macro_rules! parse_identifiers {
    ($parser:expr,
     $($str:expr => $val:expr,)+) => {
        {
            let loc = $parser.current_source_location();
            let token = $parser.next()?;

            match token {
                $(cssparser::Token::Ident(ref cow) if cow.eq_ignore_ascii_case($str) => Ok($val),)+

                _ => Err(loc.new_basic_unexpected_token_error(token.clone()))
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_number_optional_number() {
        assert_eq!(
            NumberOptionalNumber::parse_str("1, 2"),
            Ok(NumberOptionalNumber(1.0, 2.0))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1 2"),
            Ok(NumberOptionalNumber(1.0, 2.0))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1"),
            Ok(NumberOptionalNumber(1.0, 1.0))
        );

        assert_eq!(
            NumberOptionalNumber::parse_str("-1, -2"),
            Ok(NumberOptionalNumber(-1.0, -2.0))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1 -2"),
            Ok(NumberOptionalNumber(-1.0, -2.0))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1"),
            Ok(NumberOptionalNumber(-1.0, -1.0))
        );
    }

    #[test]
    fn invalid_number_optional_number() {
        assert!(NumberOptionalNumber::<f64>::parse_str("").is_err());
        assert!(NumberOptionalNumber::<f64>::parse_str("1x").is_err());
        assert!(NumberOptionalNumber::<f64>::parse_str("x1").is_err());
        assert!(NumberOptionalNumber::<f64>::parse_str("1 x").is_err());
        assert!(NumberOptionalNumber::<f64>::parse_str("1 , x").is_err());
        assert!(NumberOptionalNumber::<f64>::parse_str("1 , 2x").is_err());
        assert!(NumberOptionalNumber::<f64>::parse_str("1 2 x").is_err());
    }

    #[test]
    fn parses_integer() {
        assert_eq!(i32::parse_str("1"), Ok(1));
        assert_eq!(i32::parse_str("-1"), Ok(-1));
    }

    #[test]
    fn invalid_integer() {
        assert!(i32::parse_str("").is_err());
        assert!(i32::parse_str("1x").is_err());
        assert!(i32::parse_str("1.5").is_err());
    }

    #[test]
    fn parses_integer_optional_integer() {
        assert_eq!(
            NumberOptionalNumber::parse_str("1, 2"),
            Ok(NumberOptionalNumber(1, 2))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1 2"),
            Ok(NumberOptionalNumber(1, 2))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1"),
            Ok(NumberOptionalNumber(1, 1))
        );

        assert_eq!(
            NumberOptionalNumber::parse_str("-1, -2"),
            Ok(NumberOptionalNumber(-1, -2))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1 -2"),
            Ok(NumberOptionalNumber(-1, -2))
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1"),
            Ok(NumberOptionalNumber(-1, -1))
        );
    }

    #[test]
    fn invalid_integer_optional_integer() {
        assert!(NumberOptionalNumber::<i32>::parse_str("").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1x").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("x1").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1 x").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1 , x").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1 , 2x").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1 2 x").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1.5").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1 2.5").is_err());
        assert!(NumberOptionalNumber::<i32>::parse_str("1, 2.5").is_err());
    }
}
