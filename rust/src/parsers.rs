use ::libc;
use ::cssparser::{Parser, ParserInput, Token, BasicParseError};
use ::glib::translate::*;
use ::glib_sys;

use std::f64::consts::*;
use std::mem;
use std::ptr;
use std::slice;
use std::str;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub display: String
}

impl ParseError {
    pub fn new<T: AsRef<str>> (msg: T) -> ParseError {
        ParseError { display: msg.as_ref ().to_string () }
    }
}

impl<'a> From<BasicParseError<'a>> for ParseError {
    fn from (_: BasicParseError) -> ParseError {
        ParseError::new ("parse error")
    }
}

pub trait Parse: Sized {
    type Data;
    type Err;

    fn parse (s: &str, data: Self::Data) -> Result<Self, Self::Err>;
}

// angle:
// https://www.w3.org/TR/SVG/types.html#DataTypeAngle
//
// angle ::= number ("deg" | "grad" | "rad")?
//
// Returns an f64 angle in degrees

pub fn angle_degrees (s: &str) -> Result <f64, ParseError> {
    let mut input = ParserInput::new (s);
    let mut parser = Parser::new (&mut input);

    let angle = {
        let token = parser.next ()
            .map_err (|_| ParseError::new ("expected angle"))?;

        match *token {
            Token::Number { value, .. } => f64::from(value),

            Token::Dimension { value, ref unit, .. } => {
                let value = f64::from(value);

                match unit.as_ref () {
                    "deg"  => value,
                    "grad" => value * 360.0 / 400.0,
                    "rad"  => value * 180.0 / PI,
                    _      => return Err (ParseError::new ("expected 'deg' | 'grad' | 'rad'"))
                }
            },

            _ => return Err (ParseError::new ("expected angle"))
        }
    };

    parser.expect_exhausted ().map_err (|_| ParseError::new ("expected angle"))?;

    Ok (angle)
}

fn optional_comma (parser: &mut Parser) {
    let _ = parser.try (|p| p.expect_comma ());
}


// number-optional-number
//
// https://www.w3.org/TR/SVG/types.html#DataTypeNumberOptionalNumber

pub fn number_optional_number (s: &str) -> Result <(f64, f64), ParseError> {
    let mut input = ParserInput::new (s);
    let mut parser = Parser::new (&mut input);

    let x = f64::from(parser.expect_number ()?);

    if !parser.is_exhausted () {
        let position = parser.position ();

        match *parser.next ()? {
            Token::Comma => {},
            _ => parser.reset (position)
        };

        let y = f64::from(parser.expect_number ()?);

        parser.expect_exhausted ()?;

        Ok ((x, y))
    } else {
        Ok ((x, x))
    }
}

#[no_mangle]
pub extern fn rsvg_css_parse_number_optional_number (s: *const libc::c_char,
                                                     out_x: *mut f64,
                                                     out_y: *mut f64) -> glib_sys::gboolean {
    assert! (!s.is_null ());
    assert! (!out_x.is_null ());
    assert! (!out_y.is_null ());

    let string = unsafe { String::from_glib_none (s) };

    match number_optional_number (&string) {
        Ok ((x, y)) => {
            unsafe {
                *out_x = x;
                *out_y = y;
            }
            true
        },

        Err (_) => {
            unsafe {
                *out_x = 0.0;
                *out_y = 0.0;
            }
            false
        }
    }.to_glib ()
}


// Parse a list-of-points as for polyline and polygon elements
// https://www.w3.org/TR/SVG/shapes.html#PointsBNF

pub fn list_of_points (string: &str) -> Result <Vec<(f64, f64)>, ParseError> {
    let mut input = ParserInput::new (string);
    let mut parser = Parser::new (&mut input);

    let mut v = Vec::new ();

    loop {
        let x = f64::from(parser.expect_number ()?);

        optional_comma (&mut parser);

        let y = f64::from(parser.expect_number ()?);

        v.push ((x, y));

        if parser.is_exhausted () {
            break;
        }

        match parser.next_including_whitespace () {
            Ok (&Token::WhiteSpace(_)) => (),
            _ => optional_comma (&mut parser)
        }
    }

    Ok (v)
}

// Lists of number values

pub enum ListLength {
    Exact (usize),
    Maximum (usize)
}

#[derive(Debug, PartialEq)]
pub enum NumberListError {
    IncorrectNumberOfElements,
    Parse (ParseError)
}

pub fn number_list (s: &str, length: ListLength) -> Result <Vec<f64>, NumberListError> {
    let n;

    match length {
        ListLength::Exact (l)   => { assert! (l > 0); n = l; }
        ListLength::Maximum (l) => { assert! (l > 0); n = l; }
    }

    let mut input = ParserInput::new (s);
    let mut parser = Parser::new (&mut input);

    let mut v = Vec::<f64>::with_capacity (n);

    for i in 0..n {
        v.push (f64::from(parser.expect_number ().map_err (|_| NumberListError::Parse (ParseError::new ("expected number")))?));

        if i != n - 1 {
            optional_comma (&mut parser);
        }

        if parser.is_exhausted () {
            if let ListLength::Maximum (_) = length {
                break;
            }
        }
    }

    parser.expect_exhausted ().map_err (|_| NumberListError::IncorrectNumberOfElements)?;

    Ok(v)
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NumberListLength {
    Exact,
    Maximum
}

#[no_mangle]
pub extern fn rsvg_css_parse_number_list (in_str:   *const libc::c_char,
                                          nlength:  NumberListLength,
                                          size:     libc::size_t,
                                          out_list: *mut *const libc::c_double,
                                          out_list_length: *mut libc::size_t) -> glib_sys::gboolean {
    assert! (!in_str.is_null ());
    assert! (!out_list.is_null ());
    assert! (!out_list_length.is_null ());

    let length = match nlength {
        NumberListLength::Exact   => ListLength::Exact (size),
        NumberListLength::Maximum => ListLength::Maximum (size)
    };

    let s = unsafe { String::from_glib_none (in_str) };

    let result = number_list (&s, length);

    match result {
        Ok (number_list) => {
            let num_elems = number_list.len ();

            let c_array = unsafe {
                glib_sys::g_malloc_n (num_elems,
                                      mem::size_of::<libc::c_double> ())
                    as *mut libc::c_double
            };

            let array = unsafe { slice::from_raw_parts_mut (c_array, num_elems) };

            array.copy_from_slice (&number_list);

            unsafe {
                *out_list = c_array;
                *out_list_length = num_elems;
            }

            true
        },

        Err (_) => {
            unsafe {
                *out_list = ptr::null ();
                *out_list_length = 0;
            }
            false
        }
    }.to_glib ()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_number_optional_number () {
        assert_eq! (number_optional_number ("1, 2"), Ok ((1.0, 2.0)));
        assert_eq! (number_optional_number ("1 2"),  Ok ((1.0, 2.0)));
        assert_eq! (number_optional_number ("1"),    Ok ((1.0, 1.0)));

        assert_eq! (number_optional_number ("-1, -2"), Ok ((-1.0, -2.0)));
        assert_eq! (number_optional_number ("-1 -2"),  Ok ((-1.0, -2.0)));
        assert_eq! (number_optional_number ("-1"),     Ok ((-1.0, -1.0)));
    }

    #[test]
    fn invalid_number_optional_number () {
        assert! (number_optional_number ("").is_err ());
        assert! (number_optional_number ("1x").is_err ());
        assert! (number_optional_number ("x1").is_err ());
        assert! (number_optional_number ("1 x").is_err ());
        assert! (number_optional_number ("1 , x").is_err ());
        assert! (number_optional_number ("1 , 2x").is_err ());
        assert! (number_optional_number ("1 2 x").is_err ());
    }

    #[test]
    fn parses_list_of_points () {
        assert_eq! (list_of_points (" 1 2 "),      Ok (vec! [(1.0, 2.0)]));
        assert_eq! (list_of_points ("1 2 3 4"),  Ok (vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points ("1,2,3,4"),  Ok (vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points ("1,2 3,4"),  Ok (vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points ("1,2 -3,4"), Ok (vec! [(1.0, 2.0), (-3.0, 4.0)]));
        assert_eq! (list_of_points ("1,2,-3,4"), Ok (vec! [(1.0, 2.0), (-3.0, 4.0)]));
    }

    #[test]
    fn errors_on_invalid_list_of_points () {
        assert! (list_of_points ("-1-2-3-4").is_err ());
        assert! (list_of_points ("1 2-3,-4").is_err ());
    }

    #[test]
    fn parses_angle () {
        assert_eq! (angle_degrees ("0"),        Ok (0.0));
        assert_eq! (angle_degrees ("15"),       Ok (15.0));
        assert_eq! (angle_degrees ("180.5deg"), Ok (180.5));
        assert_eq! (angle_degrees ("1rad"),     Ok (180.0 / PI));
        assert_eq! (angle_degrees ("-400grad"), Ok (-360.0));

        assert! (angle_degrees ("").is_err ());
        assert! (angle_degrees ("foo").is_err ());
        assert! (angle_degrees ("300foo").is_err ());
    }

    #[test]
    fn parses_number_list () {
        assert_eq! (number_list ("5", ListLength::Exact (1)),
                    Ok (vec! [5.0]));

        assert_eq! (number_list ("1 2 3 4", ListLength::Exact (4)),
                    Ok (vec! [1.0, 2.0, 3.0, 4.0]));

        assert_eq! (number_list ("5", ListLength::Maximum (1)),
                    Ok (vec! [5.0]));

        assert_eq! (number_list ("1.0, -2.5", ListLength::Maximum (2)),
                    Ok (vec! [1.0, -2.5]));

        assert_eq! (number_list ("5 6", ListLength::Maximum (3)),
                    Ok (vec! [5.0, 6.0]));
    }

    #[test]
    fn errors_on_invalid_number_list () {
        // empty
        assert! (number_list ("", ListLength::Exact (1)).is_err ());

        // garbage
        assert! (number_list ("foo", ListLength::Exact (1)).is_err ());
        assert! (number_list ("1foo", ListLength::Exact (2)).is_err ());
        assert! (number_list ("1 foo", ListLength::Exact (2)).is_err ());
        assert! (number_list ("1 foo 2", ListLength::Exact (2)).is_err ());
        assert! (number_list ("1,foo", ListLength::Exact (2)).is_err ());

        // too many
        assert! (number_list ("1 2", ListLength::Exact (1)).is_err ());
        assert! (number_list ("1,2,3", ListLength::Maximum (2)).is_err ());

        // extra token
        assert! (number_list ("1,", ListLength::Exact (1)).is_err ());

        // too few
        assert! (number_list ("1", ListLength::Exact (2)).is_err ());
        assert! (number_list ("1 2", ListLength::Exact (3)).is_err ());
    }
}
