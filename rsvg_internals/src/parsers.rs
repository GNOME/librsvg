use cssparser::{BasicParseError, Parser, ParserInput, Token};

use std::str;

use crate::attributes::Attribute;
use crate::error::{NodeError, ValueErrorKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub display: String,
}

impl ParseError {
    pub fn new<T: AsRef<str>>(msg: T) -> ParseError {
        ParseError {
            display: msg.as_ref().to_string(),
        }
    }
}

impl<'a> From<BasicParseError<'a>> for ParseError {
    fn from(_: BasicParseError<'_>) -> ParseError {
        ParseError::new("parse error")
    }
}

pub trait Parse: Sized {
    type Err;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Self, Self::Err>;

    fn parse_str(s: &str) -> Result<Self, Self::Err> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        Self::parse(&mut parser).and_then(|r| {
            // FIXME: parser.expect_exhausted()?;
            Ok(r)
        })
    }
}

pub trait CssParserExt {
    /// Avoid infinities.
    fn expect_finite_number(&mut self) -> Result<f32, ValueErrorKind>;

    fn optional_comma(&mut self);
}

impl<'i, 't> CssParserExt for Parser<'i, 't> {
    fn expect_finite_number(&mut self) -> Result<f32, ValueErrorKind> {
        finite_f32(self.expect_number()?)
    }

    fn optional_comma(&mut self) {
        let _ = self.r#try(|p| p.expect_comma());
    }
}

pub fn finite_f32(n: f32) -> Result<f32, ValueErrorKind> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(ValueErrorKind::Value("expected finite number".to_string()))
    }
}

pub trait ParseValue<T: Parse<Err = ValueErrorKind>> {
    /// Parses a `value` string into a type `T`.
    fn parse(&self, value: &str) -> Result<T, NodeError>;

    /// Parses a `value` string into a type `T` with an optional validation function.
    fn parse_and_validate<F: FnOnce(T) -> Result<T, ValueErrorKind>>(
        &self,
        value: &str,
        validate: F,
    ) -> Result<T, NodeError>;
}

impl<T: Parse<Err = ValueErrorKind>> ParseValue<T> for Attribute {
    fn parse(&self, value: &str) -> Result<T, NodeError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        T::parse(&mut parser).map_err(|e| NodeError::attribute_error(*self, e))
    }

    fn parse_and_validate<F: FnOnce(T) -> Result<T, ValueErrorKind>>(
        &self,
        value: &str,
        validate: F,
    ) -> Result<T, NodeError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        T::parse(&mut parser)
            .and_then(validate)
            .map_err(|e| NodeError::attribute_error(*self, e))
    }
}

impl Parse for f64 {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<f64, ValueErrorKind> {
        Ok(f64::from(parser.expect_finite_number().map_err(|_| {
            ValueErrorKind::Parse(ParseError::new("expected number"))
        })?))
    }
}

// number
//
// https://www.w3.org/TR/SVG11/types.html#DataTypeNumber
pub fn number(s: &str) -> Result<f64, ValueErrorKind> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    Ok(f64::from(parser.expect_finite_number()?))
}

// number-optional-number
//
// https://www.w3.org/TR/SVG/types.html#DataTypeNumberOptionalNumber

pub fn number_optional_number(s: &str) -> Result<(f64, f64), ValueErrorKind> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    let x = f64::from(parser.expect_finite_number()?);

    if !parser.is_exhausted() {
        let state = parser.state();

        match *parser.next()? {
            Token::Comma => {}
            _ => parser.reset(&state),
        };

        let y = f64::from(parser.expect_finite_number()?);

        parser.expect_exhausted()?;

        Ok((x, y))
    } else {
        Ok((x, x))
    }
}

// integer
//
// https://www.w3.org/TR/SVG11/types.html#DataTypeInteger
pub fn integer(s: &str) -> Result<i32, ValueErrorKind> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    Ok(parser.expect_integer()?)
}

// integer-optional-integer
//
// Like number-optional-number but with integers.
pub fn integer_optional_integer(s: &str) -> Result<(i32, i32), ValueErrorKind> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    let x = parser.expect_integer()?;

    if !parser.is_exhausted() {
        let state = parser.state();

        match *parser.next()? {
            Token::Comma => {}
            _ => parser.reset(&state),
        };

        let y = parser.expect_integer()?;

        parser.expect_exhausted()?;

        Ok((x, y))
    } else {
        Ok((x, x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_number_optional_number() {
        assert_eq!(number_optional_number("1, 2"), Ok((1.0, 2.0)));
        assert_eq!(number_optional_number("1 2"), Ok((1.0, 2.0)));
        assert_eq!(number_optional_number("1"), Ok((1.0, 1.0)));

        assert_eq!(number_optional_number("-1, -2"), Ok((-1.0, -2.0)));
        assert_eq!(number_optional_number("-1 -2"), Ok((-1.0, -2.0)));
        assert_eq!(number_optional_number("-1"), Ok((-1.0, -1.0)));
    }

    #[test]
    fn invalid_number_optional_number() {
        assert!(number_optional_number("").is_err());
        assert!(number_optional_number("1x").is_err());
        assert!(number_optional_number("x1").is_err());
        assert!(number_optional_number("1 x").is_err());
        assert!(number_optional_number("1 , x").is_err());
        assert!(number_optional_number("1 , 2x").is_err());
        assert!(number_optional_number("1 2 x").is_err());
    }

    #[test]
    fn parses_integer() {
        assert_eq!(integer("1"), Ok(1));
        assert_eq!(integer("-1"), Ok(-1));
    }

    #[test]
    fn invalid_integer() {
        assert!(integer("").is_err());
        assert!(integer("1x").is_err());
        assert!(integer("1.5").is_err());
    }

    #[test]
    fn parses_integer_optional_integer() {
        assert_eq!(integer_optional_integer("1, 2"), Ok((1, 2)));
        assert_eq!(integer_optional_integer("1 2"), Ok((1, 2)));
        assert_eq!(integer_optional_integer("1"), Ok((1, 1)));

        assert_eq!(integer_optional_integer("-1, -2"), Ok((-1, -2)));
        assert_eq!(integer_optional_integer("-1 -2"), Ok((-1, -2)));
        assert_eq!(integer_optional_integer("-1"), Ok((-1, -1)));
    }

    #[test]
    fn invalid_integer_optional_integer() {
        assert!(integer_optional_integer("").is_err());
        assert!(integer_optional_integer("1x").is_err());
        assert!(integer_optional_integer("x1").is_err());
        assert!(integer_optional_integer("1 x").is_err());
        assert!(integer_optional_integer("1 , x").is_err());
        assert!(integer_optional_integer("1 , 2x").is_err());
        assert!(integer_optional_integer("1 2 x").is_err());
        assert!(integer_optional_integer("1.5").is_err());
        assert!(integer_optional_integer("1 2.5").is_err());
        assert!(integer_optional_integer("1, 2.5").is_err());
    }
}
