use nom::{IResult, is_digit, double, ErrorKind};
use std::str;

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

named! (pub comma,
        tag! (b","));

// Parse a viewBox attribute
// https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
//
// viewBox: double [,] double [,] double [,] double [,]
//
// x, y, w, h
//
// Where w and h must be nonnegative.

named! (pub view_box<(f64, f64, f64, f64)>,
        verify! (ws! (do_parse! (x: double >>
                                 opt! (comma) >>
                                 y: double >>
                                 opt! (comma) >>
                                 w: double >>
                                 opt! (comma) >>
                                 h: double >>
                                 (x, y, w, h))),
                 |(x, y, w, h)| w >= 0.0 && h >= 0.0));

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
    }
}
