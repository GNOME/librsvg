use nom::{IResult, double, is_alphabetic};
use std::str;
use std::f64::consts::*;

use parse_transform::*;

// I don't know how to copy a nom::IError for long-term storage
// (i.e. when it can no longer reference the &[u8]).  So, we explode a
// nom::IError into a simple error struct that can be passed around.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub display: String
}

impl ParseError {
    pub fn new (msg: &str) -> ParseError {
        ParseError { display: msg.to_string () }
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

fn is_alphabetic_or_dash (c: u8) -> bool {
     is_alphabetic (c) || c == '-' as u8 || c == '%' as u8
}

named! (pub number_and_units<(f64, &[u8])>,
        tuple! (double,
                take_while! (is_alphabetic_or_dash)));

pub fn angle_degrees (s: &str) -> Result <f64, ParseError> {
    let r = number_and_units (s.as_bytes ()).to_full_result ();

    match r {
        Ok ((value, unit)) => {
            match unit {
                b"deg"  => Ok (value),
                b"grad" => Ok (value * 360.0 / 400.0),
                b"rad"  => Ok (value * 180.0 / PI),
                b""     => Ok (value),
                _       => Err (ParseError::new ("expected (\"deg\", \"rad\", \"grad\")? after number"))
            }
        },

        _ => Err (ParseError::new ("expected a number"))
    }
}

// Parse a viewBox attribute
// https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
//
// viewBox: double [,] double [,] double [,] double [,]
//
// x, y, w, h
//
// Where w and h must be nonnegative.

named! (pub view_box<(f64, f64, f64, f64)>,
        ws! (do_parse! (x: double    >>
                        opt! (comma) >>
                        y: double    >>
                        opt! (comma) >>
                        w: double    >>
                        opt! (comma) >>
                        h: double    >>
                        eof! ()      >>
                        (x, y, w, h))));

// Coordinate pairs, separated by optional (whitespace-and/or comma)
//
// All of these yield (1, -2): "1 -2", "1, -2", "1-2"

named! (pub coordinate_pair<(f64, f64)>,
        do_parse! (x: double        >>
                   opt! (comma_wsp) >>
                   y: double        >>
                   (x, y)));

// Parse a list-of-points as for polyline and polygon elements
// https://www.w3.org/TR/SVG/shapes.html#PointsBNF

named! (list_of_points_impl<Vec<(f64, f64)>>,
        terminated! (separated_list! (comma_wsp, coordinate_pair),
                     eof! ()));

pub fn list_of_points (string: &[u8]) -> Result <Vec<(f64, f64)>, ParseError> {
    list_of_points_impl (string)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_view_box () {
        assert_eq! (view_box (b"1 2 3 4"), IResult::Done (&b""[..], (1.0, 2.0, 3.0, 4.0)));
        assert_eq! (view_box (b"1,2,3 4"), IResult::Done (&b""[..], (1.0, 2.0, 3.0, 4.0)));
        assert_eq! (view_box (b" 1,2,3 4 "), IResult::Done (&b""[..], (1.0, 2.0, 3.0, 4.0)));

        let result = view_box (b"1 2 3 4 5");

        match result {
            IResult::Error (_) => { },
            _ => { panic! ("{:?} should be an invalid viewBox", result); }
        }
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
        assert_eq! (list_of_points (b"1 2"),      Ok (vec! [(1.0, 2.0)]));
        assert_eq! (list_of_points (b"1 2 3 4"),  Ok (vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2,3,4"),  Ok (vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2 3,4"),  Ok (vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2 -3,4"), Ok (vec! [(1.0, 2.0), (-3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2,-3,4"), Ok (vec! [(1.0, 2.0), (-3.0, 4.0)]));
    }

    #[test]
    fn errors_on_invalid_list_of_points () {
        assert! (list_of_points (b"-1-2-3-4").is_err ());
        assert! (list_of_points (b"1 2-3,-4").is_err ());
    }

    #[test]
    fn parses_number_and_units () {
        assert_eq! (number_and_units (b"-1"), IResult::Done (&b""[..], (-1.0, &b""[..])));
        assert_eq! (number_and_units (b"0x"), IResult::Done (&b""[..], (0.0, &b"x"[..])));
        assert_eq! (number_and_units (b"-55.5x-large"), IResult::Done (&b""[..], (-55.5, &b"x-large"[..])));
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
}

#[cfg(test)]
mod parse_transform_tests {
    #[test]
    fn parses_numbers () {
        assert_eq! (parse_Num ("0"),          Ok (0.0));
        assert_eq! (parse_Num ("12345"),      Ok (12345.0));
        assert_eq! (parse_Num ("-123"),       Ok (-123.0));
        assert_eq! (parse_Num ("-123.25"),    Ok (-123.25));
        assert_eq! (parse_Num ("123.25"),     Ok (123.25));
        assert_eq! (parse_Num ("-.25"),       Ok (-0.25));
        assert_eq! (parse_Num (".25"),        Ok (0.25));
        assert_eq! (parse_Num ("-25."),       Ok (-25.0));
        assert_eq! (parse_Num ("25."),        Ok (25.0));
        assert_eq! (parse_Num ("22.5e1"),     Ok (225.0));
        assert_eq! (parse_Num ("-22.5e1"),    Ok (-225.0));
        assert_eq! (parse_Num ("-123.45e2"),  Ok (-12345.0));
        assert_eq! (parse_Num ("123.45E2"),   Ok (12345.0));
        assert_eq! (parse_Num ("-123.25e-2"), Ok (-1.2325));
        assert_eq! (parse_Num ("123.25E-2"),  Ok (1.2325));
    }
}
