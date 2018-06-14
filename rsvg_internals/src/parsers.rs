use cssparser::{BasicParseError, Parser, ParserInput, Token};
use glib::translate::*;
use glib_sys;
use libc;

use std::f64::consts::*;
use std::mem;
use std::ptr;
use std::slice;
use std::str::{self, FromStr};

use attributes::Attribute;
use error::{AttributeError, NodeError};
use util::utf8_cstr;

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
    fn from(_: BasicParseError) -> ParseError {
        ParseError::new("parse error")
    }
}

pub trait Parse: Sized {
    type Data;
    type Err;

    fn parse(parser: &mut Parser, data: Self::Data) -> Result<Self, Self::Err>;

    fn parse_str(s: &str, data: Self::Data) -> Result<Self, Self::Err> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        Self::parse(&mut parser, data).and_then(|r| {
            // FIXME: parser.expect_exhausted()?;
            Ok(r)
        })
    }
}

impl Parse for f64 {
    type Data = ();
    type Err = AttributeError;

    fn parse(parser: &mut Parser, _: ()) -> Result<f64, AttributeError> {
        Ok(f64::from(parser.expect_number().map_err(|_| {
            AttributeError::Parse(ParseError::new("expected number"))
        })?))
    }
}

impl Parse for String {
    type Data = ();
    type Err = AttributeError;

    fn parse(parser: &mut Parser, _: ()) -> Result<String, AttributeError> {
        Ok(String::from(
            parser
                .expect_string()
                .map_err(|_| AttributeError::Parse(ParseError::new("expected number")))?
                .as_ref(),
        ))
    }
}

/// Parses a `value` string into a type `T`.
///
/// Some value types need some extra `data` to be parsed.  This
/// corresponds to the `<T as Parse>::Data` associated type.  For
/// example, an `RsvgLength` has an associated `type Data =
/// LengthDir`, so to parse a length value, you could specify
/// `LengthDir::Horizontal` for `data`, for example.
pub fn parse<T>(key: &str, value: &str, data: <T as Parse>::Data) -> Result<T, NodeError>
where
    T: Parse<Err = AttributeError>,
{
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);

    T::parse(&mut parser, data)
        .map_err(|e| NodeError::attribute_error(Attribute::from_str(key).unwrap(), e))
}

/// Parses a `value` string into a type `T` with an optional validation function.
///
/// Some value types need some extra `data` to be parsed.  This
/// corresponds to the `<T as Parse>::Data` associated type.  For
/// example, an `RsvgLength` has an associated `type Data =
/// LengthDir`, so to parse a length value, you could specify
/// `LengthDir::Horizontal` for `data`, for example.
pub fn parse_and_validate<T, F>(
    key: &str,
    value: &str,
    data: <T as Parse>::Data,
    validate: F,
) -> Result<T, NodeError>
where
    T: Parse<Err = AttributeError>,
    F: FnOnce(T) -> Result<T, AttributeError>,
{
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);

    T::parse(&mut parser, data)
        .and_then(validate)
        .map_err(|e| NodeError::attribute_error(Attribute::from_str(key).unwrap(), e))
}

// angle:
// https://www.w3.org/TR/SVG/types.html#DataTypeAngle
//
// angle ::= number ("deg" | "grad" | "rad")?
//
// Returns an f64 angle in degrees

pub fn angle_degrees(parser: &mut Parser) -> Result<f64, ParseError> {
    let angle = {
        let token = parser
            .next()
            .map_err(|_| ParseError::new("expected angle"))?;

        match *token {
            Token::Number { value, .. } => f64::from(value),

            Token::Dimension {
                value, ref unit, ..
            } => {
                let value = f64::from(value);

                match unit.as_ref() {
                    "deg" => value,
                    "grad" => value * 360.0 / 400.0,
                    "rad" => value * 180.0 / PI,
                    _ => return Err(ParseError::new("expected 'deg' | 'grad' | 'rad'")),
                }
            }

            _ => return Err(ParseError::new("expected angle")),
        }
    };

    parser
        .expect_exhausted()
        .map_err(|_| ParseError::new("expected angle"))?;

    Ok(angle)
}

pub fn optional_comma(parser: &mut Parser) {
    let _ = parser.try(|p| p.expect_comma());
}

// number
//
// https://www.w3.org/TR/SVG11/types.html#DataTypeNumber
pub fn number(s: &str) -> Result<f64, ParseError> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    Ok(f64::from(parser.expect_number()?))
}

// number-optional-number
//
// https://www.w3.org/TR/SVG/types.html#DataTypeNumberOptionalNumber

pub fn number_optional_number(s: &str) -> Result<(f64, f64), ParseError> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    let x = f64::from(parser.expect_number()?);

    if !parser.is_exhausted() {
        let state = parser.state();

        match *parser.next()? {
            Token::Comma => {}
            _ => parser.reset(&state),
        };

        let y = f64::from(parser.expect_number()?);

        parser.expect_exhausted()?;

        Ok((x, y))
    } else {
        Ok((x, x))
    }
}

#[no_mangle]
pub extern "C" fn rsvg_css_parse_number_optional_number(
    s: *const libc::c_char,
    out_x: *mut f64,
    out_y: *mut f64,
) -> glib_sys::gboolean {
    assert!(!s.is_null());
    assert!(!out_x.is_null());
    assert!(!out_y.is_null());

    let string = unsafe { utf8_cstr(s) };

    match number_optional_number(string) {
        Ok((x, y)) => {
            unsafe {
                *out_x = x;
                *out_y = y;
            }
            true
        }

        Err(_) => {
            unsafe {
                *out_x = 0.0;
                *out_y = 0.0;
            }
            false
        }
    }.to_glib()
}

// Parse a list-of-points as for polyline and polygon elements
// https://www.w3.org/TR/SVG/shapes.html#PointsBNF

pub fn list_of_points(string: &str) -> Result<Vec<(f64, f64)>, ParseError> {
    let mut input = ParserInput::new(string);
    let mut parser = Parser::new(&mut input);

    let mut v = Vec::new();

    loop {
        let x = f64::from(parser.expect_number()?);

        optional_comma(&mut parser);

        let y = f64::from(parser.expect_number()?);

        v.push((x, y));

        if parser.is_exhausted() {
            break;
        }

        match parser.next_including_whitespace() {
            Ok(&Token::WhiteSpace(_)) => (),
            _ => optional_comma(&mut parser),
        }
    }

    Ok(v)
}

// Lists of number values

pub enum ListLength {
    Exact(usize),
    Maximum(usize),
}

#[derive(Debug, PartialEq)]
pub enum NumberListError {
    IncorrectNumberOfElements,
    Parse(ParseError),
}

pub fn number_list(parser: &mut Parser, length: ListLength) -> Result<Vec<f64>, NumberListError> {
    let n;

    match length {
        ListLength::Exact(l) => {
            assert!(l > 0);
            n = l;
        }
        ListLength::Maximum(l) => {
            assert!(l > 0);
            n = l;
        }
    }

    let mut v = Vec::<f64>::with_capacity(n);

    for i in 0..n {
        v.push(f64::from(parser.expect_number().map_err(|_| {
            NumberListError::Parse(ParseError::new("expected number"))
        })?));

        if i != n - 1 {
            optional_comma(parser);
        }

        if parser.is_exhausted() {
            if let ListLength::Maximum(_) = length {
                break;
            }
        }
    }

    parser
        .expect_exhausted()
        .map_err(|_| NumberListError::IncorrectNumberOfElements)?;

    Ok(v)
}

fn number_list_from_str(s: &str, length: ListLength) -> Result<Vec<f64>, NumberListError> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    number_list(&mut parser, length)
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NumberListLength {
    Exact,
    Maximum,
}

#[no_mangle]
pub extern "C" fn rsvg_css_parse_number_list(
    in_str: *const libc::c_char,
    nlength: NumberListLength,
    size: libc::size_t,
    out_list: *mut *const libc::c_double,
    out_list_length: *mut libc::size_t,
) -> glib_sys::gboolean {
    assert!(!in_str.is_null());
    assert!(!out_list.is_null());
    assert!(!out_list_length.is_null());

    let length = match nlength {
        NumberListLength::Exact => ListLength::Exact(size),
        NumberListLength::Maximum => ListLength::Maximum(size),
    };

    let s = unsafe { utf8_cstr(in_str) };

    let result = number_list_from_str(s, length);

    match result {
        Ok(number_list) => {
            let num_elems = number_list.len();

            let c_array = unsafe {
                glib_sys::g_malloc_n(num_elems, mem::size_of::<libc::c_double>())
                    as *mut libc::c_double
            };

            let array = unsafe { slice::from_raw_parts_mut(c_array, num_elems) };

            array.copy_from_slice(&number_list);

            unsafe {
                *out_list = c_array;
                *out_list_length = num_elems;
            }

            true
        }

        Err(_) => {
            unsafe {
                *out_list = ptr::null();
                *out_list_length = 0;
            }
            false
        }
    }.to_glib()
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
    fn parses_list_of_points() {
        assert_eq!(list_of_points(" 1 2 "), Ok(vec![(1.0, 2.0)]));
        assert_eq!(list_of_points("1 2 3 4"), Ok(vec![(1.0, 2.0), (3.0, 4.0)]));
        assert_eq!(list_of_points("1,2,3,4"), Ok(vec![(1.0, 2.0), (3.0, 4.0)]));
        assert_eq!(list_of_points("1,2 3,4"), Ok(vec![(1.0, 2.0), (3.0, 4.0)]));
        assert_eq!(
            list_of_points("1,2 -3,4"),
            Ok(vec![(1.0, 2.0), (-3.0, 4.0)])
        );
        assert_eq!(
            list_of_points("1,2,-3,4"),
            Ok(vec![(1.0, 2.0), (-3.0, 4.0)])
        );
    }

    #[test]
    fn errors_on_invalid_list_of_points() {
        assert!(list_of_points("-1-2-3-4").is_err());
        assert!(list_of_points("1 2-3,-4").is_err());
    }

    fn angle_degrees_str(s: &str) -> Result<f64, ParseError> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        angle_degrees(&mut parser)
    }

    #[test]
    fn parses_angle() {
        assert_eq!(angle_degrees_str("0"), Ok(0.0));
        assert_eq!(angle_degrees_str("15"), Ok(15.0));
        assert_eq!(angle_degrees_str("180.5deg"), Ok(180.5));
        assert_eq!(angle_degrees_str("1rad"), Ok(180.0 / PI));
        assert_eq!(angle_degrees_str("-400grad"), Ok(-360.0));

        assert!(angle_degrees_str("").is_err());
        assert!(angle_degrees_str("foo").is_err());
        assert!(angle_degrees_str("300foo").is_err());
    }

    #[test]
    fn parses_number_list() {
        assert_eq!(
            number_list_from_str("5", ListLength::Exact(1)),
            Ok(vec![5.0])
        );

        assert_eq!(
            number_list_from_str("1 2 3 4", ListLength::Exact(4)),
            Ok(vec![1.0, 2.0, 3.0, 4.0])
        );

        assert_eq!(
            number_list_from_str("5", ListLength::Maximum(1)),
            Ok(vec![5.0])
        );

        assert_eq!(
            number_list_from_str("1.0, -2.5", ListLength::Maximum(2)),
            Ok(vec![1.0, -2.5])
        );

        assert_eq!(
            number_list_from_str("5 6", ListLength::Maximum(3)),
            Ok(vec![5.0, 6.0])
        );
    }

    #[test]
    fn errors_on_invalid_number_list() {
        // empty
        assert!(number_list_from_str("", ListLength::Exact(1)).is_err());

        // garbage
        assert!(number_list_from_str("foo", ListLength::Exact(1)).is_err());
        assert!(number_list_from_str("1foo", ListLength::Exact(2)).is_err());
        assert!(number_list_from_str("1 foo", ListLength::Exact(2)).is_err());
        assert!(number_list_from_str("1 foo 2", ListLength::Exact(2)).is_err());
        assert!(number_list_from_str("1,foo", ListLength::Exact(2)).is_err());

        // too many
        assert!(number_list_from_str("1 2", ListLength::Exact(1)).is_err());
        assert!(number_list_from_str("1,2,3", ListLength::Maximum(2)).is_err());

        // extra token
        assert!(number_list_from_str("1,", ListLength::Exact(1)).is_err());

        // too few
        assert!(number_list_from_str("1", ListLength::Exact(2)).is_err());
        assert!(number_list_from_str("1 2", ListLength::Exact(3)).is_err());
    }
}
