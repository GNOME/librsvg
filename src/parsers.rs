//! The `Parse` trait for CSS properties, and utilities for parsers.

use cssparser::{Parser, ParserInput, Token};
use markup5ever::QualName;
use std::str;

use crate::error::*;

/// Trait to parse values using `cssparser::Parser`.
pub trait Parse: Sized {
    /// Parses a value out of the `parser`.
    ///
    /// All value types should implement this for composability.
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>>;

    /// Convenience function to parse a value out of a `&str`.
    ///
    /// This is useful mostly for tests which want to avoid creating a
    /// `cssparser::Parser` by hand.  Property types do not need to reimplement this.
    fn parse_str(s: &str) -> Result<Self, ParseError<'_>> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        let res = Self::parse(&mut parser)?;
        parser.expect_exhausted()?;

        Ok(res)
    }
}

/// Consumes a comma if it exists, or does nothing.
pub fn optional_comma<'i, 't>(parser: &mut Parser<'i, 't>) {
    let _ = parser.try_parse(|p| p.expect_comma());
}

/// Parses an `f32` and ensures that it is not an infinity or NaN.
pub fn finite_f32(n: f32) -> Result<f32, ValueErrorKind> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(ValueErrorKind::Value("expected finite number".to_string()))
    }
}

pub trait ParseValue<T: Parse> {
    /// Parses a `value` string into a type `T`.
    fn parse(&self, value: &str) -> Result<T, ElementError>;
}

impl<T: Parse> ParseValue<T> for QualName {
    fn parse(&self, value: &str) -> Result<T, ElementError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        T::parse(&mut parser).attribute(self.clone())
    }
}

impl<T: Parse> Parse for Option<T> {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        T::parse(parser).map(Some)
    }
}

impl Parse for f64 {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let loc = parser.current_source_location();
        let n = parser.expect_number()?;
        if n.is_finite() {
            Ok(f64::from(n))
        } else {
            Err(loc.new_custom_error(ValueErrorKind::value_error("expected finite number")))
        }
    }
}

/// Non-Negative number
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NonNegative(pub f64);

impl Parse for NonNegative {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let loc = parser.current_source_location();
        let n = Parse::parse(parser)?;
        if n >= 0.0 {
            Ok(NonNegative(n))
        } else {
            Err(loc.new_custom_error(ValueErrorKind::value_error("expected non negative number")))
        }
    }
}

/// CSS number-optional-number
///
/// https://www.w3.org/TR/SVG/types.html#DataTypeNumberOptionalNumber
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NumberOptionalNumber<T: Parse>(pub T, pub T);

impl<T: Parse + Copy> Parse for NumberOptionalNumber<T> {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let x = Parse::parse(parser)?;

        if !parser.is_exhausted() {
            optional_comma(parser);
            let y = Parse::parse(parser)?;
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
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parser.expect_integer()?)
    }
}

impl Parse for u32 {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let loc = parser.current_source_location();
        let n = parser.expect_integer()?;
        if n >= 0 {
            Ok(n as u32)
        } else {
            Err(loc.new_custom_error(ValueErrorKind::value_error("expected unsigned number")))
        }
    }
}

#[derive(Eq, PartialEq)]
pub enum NumberListLength {
    Exact(usize),
    Unbounded,
}

#[derive(Debug, PartialEq)]
pub struct NumberList(pub Vec<f64>);

/// CSS number-list values.
impl NumberList {
    pub fn parse<'i>(
        parser: &mut Parser<'i, '_>,
        length: NumberListLength,
    ) -> Result<Self, ParseError<'i>> {
        let mut v = match length {
            NumberListLength::Exact(l) if l > 0 => Vec::<f64>::with_capacity(l),
            NumberListLength::Exact(_) => unreachable!("NumberListLength::Exact cannot be 0"),
            NumberListLength::Unbounded => Vec::<f64>::new(),
        };

        if parser.is_exhausted() && length == NumberListLength::Unbounded {
            return Ok(NumberList(v));
        }

        for i in 0.. {
            if i != 0 {
                optional_comma(parser);
            }

            v.push(f64::parse(parser)?);

            if let NumberListLength::Exact(l) = length {
                if i + 1 == l {
                    break;
                }
            }

            if parser.is_exhausted() {
                match length {
                    NumberListLength::Exact(l) => {
                        if i + 1 == l {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }

        Ok(NumberList(v))
    }

    pub fn parse_str(s: &str, length: NumberListLength) -> Result<NumberList, ParseError<'_>> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        let res = Self::parse(&mut parser, length)?;
        parser.expect_exhausted()?;
        Ok(res)
    }
}

/// Parses a list of identifiers from a `cssparser::Parser`
///
/// # Example
///
/// ```
/// # #[macro_use] extern crate librsvg;
/// # use cssparser::{ParserInput, Parser};
/// # fn main() -> Result<(), cssparser::BasicParseError<'static>> {
/// # let mut input = ParserInput::new("true");
/// # let mut parser = Parser::new(&mut input);
/// let my_boolean = parse_identifiers!(
///     parser,
///     "true" => true,
///     "false" => false,
/// )?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
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

/// https://www.w3.org/TR/css-values-4/#custom-idents
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomIdent(String);

impl Parse for CustomIdent {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let loc = parser.current_source_location();
        let token = parser.next()?;

        match token {
            // CSS-wide keywords and "default" are errors here
            // https://www.w3.org/TR/css-values-4/#css-wide-keywords
            Token::Ident(ref cow) => {
                for s in &["initial", "inherit", "unset", "default"] {
                    if cow.eq_ignore_ascii_case(s) {
                        return Err(loc.new_basic_unexpected_token_error(token.clone()).into());
                    }
                }

                Ok(CustomIdent(cow.as_ref().to_string()))
            }

            _ => Err(loc.new_basic_unexpected_token_error(token.clone()).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_number_optional_number() {
        assert_eq!(
            NumberOptionalNumber::parse_str("1, 2").unwrap(),
            NumberOptionalNumber(1.0, 2.0)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1 2").unwrap(),
            NumberOptionalNumber(1.0, 2.0)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1").unwrap(),
            NumberOptionalNumber(1.0, 1.0)
        );

        assert_eq!(
            NumberOptionalNumber::parse_str("-1, -2").unwrap(),
            NumberOptionalNumber(-1.0, -2.0)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1 -2").unwrap(),
            NumberOptionalNumber(-1.0, -2.0)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1").unwrap(),
            NumberOptionalNumber(-1.0, -1.0)
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
        assert_eq!(i32::parse_str("0").unwrap(), 0);
        assert_eq!(i32::parse_str("1").unwrap(), 1);
        assert_eq!(i32::parse_str("-1").unwrap(), -1);

        assert_eq!(u32::parse_str("0").unwrap(), 0);
        assert_eq!(u32::parse_str("1").unwrap(), 1);
    }

    #[test]
    fn invalid_integer() {
        assert!(i32::parse_str("").is_err());
        assert!(i32::parse_str("1x").is_err());
        assert!(i32::parse_str("1.5").is_err());

        assert!(u32::parse_str("").is_err());
        assert!(u32::parse_str("1x").is_err());
        assert!(u32::parse_str("1.5").is_err());
        assert!(u32::parse_str("-1").is_err());
    }

    #[test]
    fn parses_integer_optional_integer() {
        assert_eq!(
            NumberOptionalNumber::parse_str("1, 2").unwrap(),
            NumberOptionalNumber(1, 2)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1 2").unwrap(),
            NumberOptionalNumber(1, 2)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("1").unwrap(),
            NumberOptionalNumber(1, 1)
        );

        assert_eq!(
            NumberOptionalNumber::parse_str("-1, -2").unwrap(),
            NumberOptionalNumber(-1, -2)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1 -2").unwrap(),
            NumberOptionalNumber(-1, -2)
        );
        assert_eq!(
            NumberOptionalNumber::parse_str("-1").unwrap(),
            NumberOptionalNumber(-1, -1)
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

    #[test]
    fn parses_number_list() {
        assert_eq!(
            NumberList::parse_str("5", NumberListLength::Exact(1)).unwrap(),
            NumberList(vec![5.0])
        );

        assert_eq!(
            NumberList::parse_str("1 2 3 4", NumberListLength::Exact(4)).unwrap(),
            NumberList(vec![1.0, 2.0, 3.0, 4.0])
        );

        assert_eq!(
            NumberList::parse_str("", NumberListLength::Unbounded).unwrap(),
            NumberList(vec![])
        );

        assert_eq!(
            NumberList::parse_str("1, 2, 3.0, 4, 5", NumberListLength::Unbounded).unwrap(),
            NumberList(vec![1.0, 2.0, 3.0, 4.0, 5.0])
        );
    }

    #[test]
    fn errors_on_invalid_number_list() {
        // empty
        assert!(NumberList::parse_str("", NumberListLength::Exact(1)).is_err());

        // garbage
        assert!(NumberList::parse_str("foo", NumberListLength::Exact(1)).is_err());
        assert!(NumberList::parse_str("1foo", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1 foo", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1 foo 2", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1,foo", NumberListLength::Exact(2)).is_err());

        // too many
        assert!(NumberList::parse_str("1 2", NumberListLength::Exact(1)).is_err());

        // extra token
        assert!(NumberList::parse_str("1,", NumberListLength::Exact(1)).is_err());
        assert!(NumberList::parse_str("1,", NumberListLength::Exact(1)).is_err());
        assert!(NumberList::parse_str("1,", NumberListLength::Unbounded).is_err());

        // too few
        assert!(NumberList::parse_str("1", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1 2", NumberListLength::Exact(3)).is_err());
    }

    #[test]
    fn parses_custom_ident() {
        assert_eq!(
            CustomIdent::parse_str("hello").unwrap(),
            CustomIdent("hello".to_string())
        );
    }

    #[test]
    fn invalid_custom_ident_yields_error() {
        assert!(CustomIdent::parse_str("initial").is_err());
        assert!(CustomIdent::parse_str("inherit").is_err());
        assert!(CustomIdent::parse_str("unset").is_err());
        assert!(CustomIdent::parse_str("default").is_err());
        assert!(CustomIdent::parse_str("").is_err());
    }
}
