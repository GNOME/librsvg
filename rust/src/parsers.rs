use nom::{IResult, is_digit, double, is_alphabetic};
use std::str;
use std::f64::consts::*;

#[derive(Debug, PartialEq, Eq)]
pub enum Sign {
    Plus,
    Minus,
}

named! (pub sign<Sign>,
        map! (alt! (tag! (b"+") | tag! (b"-")),
              |x| if x == b"+" { Sign::Plus } else { Sign::Minus }
        )
);

type DigitSequence<'a> = &'a[u8];
named! (pub digit_sequence<DigitSequence>, take_while1! (is_digit));

type Exponent<'a> = (Sign, &'a[u8]);
named! (pub exponent<Exponent>,
        do_parse! (alt! (tag! (b"e") | tag! (b"E")) >>
                   s: opt! (sign)                   >>
                   d: digit_sequence                >>
                   (s.unwrap_or (Sign::Plus), d)
        )
);

type FractionalConstant<'a> = (Option<&'a[u8]>, Option<&'a[u8]>);
named! (pub fractional_constant<FractionalConstant>,
        alt! (do_parse! (i: opt! (digit_sequence) >>
                         tag! (b".")              >>
                         f: digit_sequence        >>
                         (i, Some (f)))           |

              do_parse! (i: digit_sequence        >>
                         tag! (b".")              >>
                         (Some(i), None)))
);

type FloatingPointConstant<'a> = (FractionalConstant<'a>, Option<Exponent<'a>>);
named! (pub floating_point_constant<FloatingPointConstant>,
        alt! (do_parse! (f: fractional_constant        >>
                         e: opt! (exponent)            >>
                         (f, e))                       |

              do_parse! (d: digit_sequence             >>
                         e: exponent                   >>
                         ((Some (d), None), Some (e))))
);

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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ParseAngleError;

fn is_alphabetic_or_dash (c: u8) -> bool {
     is_alphabetic (c) || c == '-' as u8 || c == '%' as u8
}

named! (pub number_and_units<(f64, &[u8])>,
        tuple! (double,
                take_while! (is_alphabetic_or_dash)));

pub fn angle_degrees (s: &str) -> Result <f64, ParseAngleError> {
    let r = number_and_units (s.as_bytes ()).to_full_result ();

    match r {
        Ok ((value, unit)) => {
            match unit {
                b"deg"  => Ok (value),
                b"grad" => Ok (value * 360.0 / 400.0),
                b"rad"  => Ok (value * 180.0 / PI),
                b""     => Ok (value),
                _       => Err (ParseAngleError)
            }
        },

        _ => Err (ParseAngleError)
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

named! (pub list_of_points<Vec<(f64, f64)>>,
        terminated! (separated_list! (comma_wsp, coordinate_pair),
                     eof! ()));

named! (pub separated_numbers<Vec<f64>>,
        separated_list! (comma_wsp, double));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works () {
        assert_eq! (sign (b"+"), IResult::Done (&b""[..], Sign::Plus));
        assert_eq! (sign (b"-"), IResult::Done (&b""[..], Sign::Minus));
        assert_eq! (digit_sequence (b"123456"), IResult::Done (&b""[..], &b"123456"[..]));
        assert_eq! (digit_sequence (b"123456b"), IResult::Done (&b"b"[..], &b"123456"[..]));
        assert_eq! (digit_sequence (b"1234b56"), IResult::Done (&b"b56"[..], &b"1234"[..]));
        assert_eq! (exponent (b"e123"), IResult::Done (&b""[..], (Sign::Plus, &b"123"[..])));
        assert_eq! (exponent (b"e+123"), IResult::Done (&b""[..], (Sign::Plus, &b"123"[..])));
        assert_eq! (exponent (b"e-123"), IResult::Done (&b""[..], (Sign::Minus, &b"123"[..])));
        assert_eq! (fractional_constant (b"1.23"), IResult::Done (&b""[..], (Some (&b"1"[..]), Some (&b"23"[..]))));
        assert_eq! (fractional_constant (b"1."), IResult::Done (&b""[..], (Some (&b"1"[..]), None)));
        assert_eq! (fractional_constant (b".23"), IResult::Done (&b""[..], (None, Some (&b"23"[..]))));
    }

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
        assert_eq! (list_of_points (b"1 2"),  IResult::Done (&b""[..], vec! [(1.0, 2.0)]));
        assert_eq! (list_of_points (b"1 2 3 4"),  IResult::Done (&b""[..], vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2,3,4"),  IResult::Done (&b""[..], vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2 3,4"),  IResult::Done (&b""[..], vec! [(1.0, 2.0), (3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2 -3,4"),  IResult::Done (&b""[..], vec! [(1.0, 2.0), (-3.0, 4.0)]));
        assert_eq! (list_of_points (b"1,2,-3,4"),  IResult::Done (&b""[..], vec! [(1.0, 2.0), (-3.0, 4.0)]));
    }

    #[test]
    fn errors_on_invalid_list_of_points () {
        let result = list_of_points (b"-1-2-3-4");
        match result {
            IResult::Error (_) => { },
            _ => { panic! ("{:?} should be an invalid list-of-points", result); }
        }

        let result = list_of_points (b"1 2-3,-4");
        match result {
            IResult::Error (_) => { },
            _ => { panic! ("{:?} should be an invalid list-of-points", result); }
        }
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

        assert_eq! (angle_degrees (""), Err (ParseAngleError));
        assert_eq! (angle_degrees ("foo"), Err (ParseAngleError));
        assert_eq! (angle_degrees ("300foo"), Err (ParseAngleError));
    }
}
