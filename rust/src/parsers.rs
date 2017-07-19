use ::libc;
use ::cssparser::{Parser, Token, BasicParseError, NumericValue};
use ::glib::translate::*;
use ::glib_sys;
use ::nom::{IResult, double};

use std::f64::consts::*;
use std::mem;
use std::ptr;
use std::slice;
use std::str;

// I don't know how to copy a nom::IError for long-term storage
// (i.e. when it can no longer reference the &[u8]).  So, we explode a
// nom::IError into a simple error struct that can be passed around.
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

/*
impl<'a> From<IError<&'a [u8]>> for NomError {
    fn from (e: IError<&[u8]>) -> NomError {
        match e {
            IError::Error (err) => NomError { display: format! ("{}", err) },

            IError::Incomplete (_) => NomError { display: "incomplete data".to_string () }
        }
    }
}
*/

fn is_whitespace (c: u8) -> bool {
    match c as char {
        ' ' | '\t' | '\r' | '\n' => true,
        _ => false
    }
}

// comma-wsp:
//     (wsp+ comma? wsp*) | (comma wsp*)

named! (comma, complete! (tag! (",")));
named! (wsp, recognize! (take_while1! (is_whitespace)));
named! (wsp_opt, recognize! (take_while! (is_whitespace)));

named! (comma_wsp,
    alt! (recognize! (tuple! (comma,
                              wsp_opt))
          | recognize! (tuple! (wsp,
                                opt! (comma),
                                wsp_opt))));

// angle:
// https://www.w3.org/TR/SVG/types.html#DataTypeAngle
//
// angle ::= number ("deg" | "grad" | "rad")?
//
// Returns an f64 angle in degrees

pub fn angle_degrees (s: &str) -> Result <f64, ParseError> {
    let mut parser = Parser::new (s);

    let token = parser.next ()
        .map_err (|_| ParseError::new ("expected angle"))?;

    match token {
        Token::Number (NumericValue { value, .. }) => Ok (value as f64),

        Token::Dimension (NumericValue { value, .. }, cow) => {
            let value = value as f64;

            match cow.as_ref () {
                "deg"  => Ok (value),
                "grad" => Ok (value * 360.0 / 400.0),
                "rad"  => Ok (value * 180.0 / PI),
                _      => Err (ParseError::new ("expected angle"))
            }
        },

        _ => Err (ParseError::new ("expected angle"))
    }.and_then (|r|
                parser.expect_exhausted ()
                .map (|_| r)
                .map_err (|_| ParseError::new ("expected angle")))
}

// Parse a viewBox attribute
// https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
//
// viewBox: double [,] double [,] double [,] double [,]
//
// x, y, w, h
//
// Where w and h must be nonnegative.

named! (parse_view_box<(f64, f64, f64, f64)>,
        ws! (do_parse! (x: double    >>
                        opt! (comma) >>
                        y: double    >>
                        opt! (comma) >>
                        w: double    >>
                        opt! (comma) >>
                        h: double    >>
                        eof! ()      >>
                        (x, y, w, h))));

pub fn view_box (s: &str) -> Result <(f64, f64, f64, f64), ParseError> {
    parse_view_box (s.as_bytes ()).to_full_result ()
        .map_err (|_| ParseError::new ("string does not match 'x [,] y [,] w [,] h'"))
}

// Coordinate pairs, separated by optional (whitespace-and/or comma)
//
// All of these yield (1, -2): "1 -2", "1, -2", "1-2"

named! (coordinate_pair<(f64, f64)>,
        do_parse! (x: double        >>
                   opt! (comma_wsp) >>
                   y: double        >>
                   (x, y)));

// number-optional-number
//
// https://www.w3.org/TR/SVG/types.html#DataTypeNumberOptionalNumber

pub fn number_optional_number (s: &str) -> Result <(f64, f64), ParseError> {
    let mut parser = Parser::new (s);

    let x = parser.expect_number ()? as f64;

    if !parser.is_exhausted () {
        let position = parser.position ();

        match parser.next ()? {
            Token::Comma => {},
            _ => parser.reset (position)
        };

        let y = parser.expect_number ()? as f64;

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

named! (list_of_points_impl<Vec<(f64, f64)>>,
        terminated! (separated_list! (comma_wsp, coordinate_pair),
                     eof! ()));

pub fn list_of_points (string: &str) -> Result <Vec<(f64, f64)>, ParseError> {
    list_of_points_impl (string.as_bytes ())
        .to_full_result ()
        .map_err (|_| ParseError::new ("invalid syntax for list of points"))
    /*
        .map_err (|e| match e { IError::Error (err) => ParseError::new (format! ("{}", err)),
                                _ => ParseError::new ("incomplete list of points")
        })
     */
}

named! (pub separated_numbers<Vec<f64>>,
        separated_list! (comma_wsp, double));

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

fn number_list (s: &str, length: ListLength) -> Result <Vec<f64>, NumberListError> {
    let n;

    match length {
        ListLength::Exact (l)   => { assert! (l > 0); n = l; }
        ListLength::Maximum (l) => { assert! (l > 0); n = l; }
    }

    let mut parser = Parser::new (s);

    let mut v = Vec::<f64>::with_capacity (n);

    for i in 0..n {
        v.push (parser.expect_number ().map_err (|_| NumberListError::Parse (ParseError::new ("expected number")))? as f64);

        if i != n - 1 {
            let _ = parser.try (|p| p.expect_comma ());
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

            let mut array = unsafe { slice::from_raw_parts_mut (c_array, num_elems) };

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
    fn parses_view_box () {
        assert_eq! (view_box ("1 2 3 4"), Ok ((1.0, 2.0, 3.0, 4.0)));
        assert_eq! (view_box ("1,2,3 4"), Ok ((1.0, 2.0, 3.0, 4.0)));
        assert_eq! (view_box (" 1,2,3 4 "), Ok ((1.0, 2.0, 3.0, 4.0)));

        assert! (view_box ("1 2 3 4 5").is_err ());
    }

    #[test]
    fn parses_coordinate_pairs () {
        assert_eq! (coordinate_pair (b"1 2"),    IResult::Done (&b""[..], (1.0, 2.0)));
        assert_eq! (coordinate_pair (b"1-2"),    IResult::Done (&b""[..], (1.0, -2.0)));
        assert_eq! (coordinate_pair (b"1,2"),    IResult::Done (&b""[..], (1.0, 2.0)));
        assert_eq! (coordinate_pair (b"1, 2"),   IResult::Done (&b""[..], (1.0, 2.0)));
        assert_eq! (coordinate_pair (b"1 ,2"),   IResult::Done (&b""[..], (1.0, 2.0)));
        assert_eq! (coordinate_pair (b"1 , 2"),  IResult::Done (&b""[..], (1.0, 2.0)));
        assert_eq! (coordinate_pair (b"1 -2"),   IResult::Done (&b""[..], (1.0, -2.0)));
        assert_eq! (coordinate_pair (b"1,-2"),   IResult::Done (&b""[..], (1.0, -2.0)));
        assert_eq! (coordinate_pair (b"1, -2"),  IResult::Done (&b""[..], (1.0, -2.0)));
        assert_eq! (coordinate_pair (b"1 , -2"), IResult::Done (&b""[..], (1.0, -2.0)));
    }

    #[test]
    fn detects_incomplete_coordinate_pair () {
        let result = coordinate_pair (b"1");
        match result {
            IResult::Incomplete (_) => { },
            _ => { panic! ("{:?} should be an incomplete coordinate-pair", result); }
        }

        let result = coordinate_pair (b"1,");
        match result {
            IResult::Incomplete (_) => { },
            _ => { panic! ("{:?} should be an incomplete coordinate-pair", result); }
        }

        let result = coordinate_pair (b"1, ");
        match result {
            IResult::Incomplete (_) => { },
            _ => { panic! ("{:?} should be an incomplete coordinate-pair", result); }
        }

        let result = coordinate_pair (b"1-");
        match result {
            IResult::Incomplete (_) => { },
            _ => { panic! ("{:?} should be an incomplete coordinate-pair", result); }
        }

        let result = coordinate_pair (b"1,-");
        match result {
            IResult::Incomplete (_) => { },
            _ => { panic! ("{:?} should be an incomplete coordinate-pair", result); }
        }

        let result = coordinate_pair (b"-1 -");
        match result {
            IResult::Incomplete (_) => { },
            _ => { panic! ("{:?} should be an incomplete coordinate-pair", result); }
        }
    }

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
    fn parses_comma_wsp () {
        assert_eq! (comma_wsp (b" , "), IResult::Done (&b""[..], &b" , "[..]));
        assert_eq! (comma_wsp (b","),   IResult::Done (&b""[..], &b","[..]));
        assert_eq! (comma_wsp (b" "),   IResult::Done (&b""[..], &b" "[..]));
        assert_eq! (comma_wsp (b", "),  IResult::Done (&b""[..], &b", "[..]));
        assert_eq! (comma_wsp (b" ,"),  IResult::Done (&b""[..], &b" ,"[..]));
    }

    #[test]
    fn parses_separated_numbers () {
        assert_eq! (separated_numbers (b"1 2 3 4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1,2,3,4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1 ,2 ,3 ,4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1  ,2  ,3  ,4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1, 2, 3, 4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1,  2,  3,  4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1 , 2 , 3 , 4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1  , 2  , 3  , 4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
        assert_eq! (separated_numbers (b"1  ,  2  ,  3  ,  4"), IResult::Done (&b""[..], vec! [1.0, 2.0, 3.0, 4.0]));
    }

    #[test]
    fn parses_list_of_points () {
        // FIXME: we are missing optional whitespace at the beginning and end of the list
        assert_eq! (list_of_points ("1 2"),      Ok (vec! [(1.0, 2.0)]));
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
