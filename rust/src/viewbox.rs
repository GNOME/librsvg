use ::cairo;
use ::glib_sys;
use ::glib;

use std::str::FromStr;

use error::*;
use parsers;
use parsers::{ListLength, ParseError};

use self::glib::translate::*;

/* Keep this in sync with rsvg-private.h:RsvgViewBox */
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RsvgViewBox {
    pub rect: cairo::Rectangle,
    active:   glib_sys::gboolean
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ViewBox(pub cairo::Rectangle);

impl From<Option<ViewBox>> for RsvgViewBox {
    fn from(v: Option<ViewBox>) -> RsvgViewBox {
        if let Some(vb) = v {
            RsvgViewBox {
                rect: vb.0,
                active: true.to_glib ()
            }
        } else {
            RsvgViewBox {
                rect: cairo::Rectangle { x: 0.0,
                                         y: 0.0,
                                         width: 0.0,
                                         height: 0.0 },
                active: false.to_glib ()
            }
        }
    }
}

impl FromStr for ViewBox {
    type Err = AttributeError;

    // Parse a viewBox attribute
    // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
    //
    // viewBox: double [,] double [,] double [,] double [,]
    //
    // x, y, w, h
    //
    // Where w and h must be nonnegative.
    fn from_str (s: &str) -> Result<ViewBox, AttributeError> {
        let v = parsers::number_list (s, ListLength::Exact (4))
            .map_err (|_| ParseError::new ("string does not match 'x [,] y [,] w [,] h'"))?;

        let (x, y, w, h) = (v[0], v[1], v[2], v[3]);

        if w >= 0.0 && h >= 0.0 {
            Ok (ViewBox(cairo::Rectangle { x: x,
                                           y: y,
                                           width: w,
                                           height: h }))
        } else {
            Err (AttributeError::Value ("width and height must not be negative".to_string ()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_valid_viewboxes () {
        assert_eq! (ViewBox::from_str ("  1 2 3 4"),
                    Ok (ViewBox (cairo::Rectangle { x: 1.0,
                                                    y: 2.0,
                                                    width: 3.0,
                                                    height: 4.0 })));

        assert_eq! (ViewBox::from_str (" -1.5 -2.5e1,34,56e2  "),
                    Ok (ViewBox (cairo::Rectangle { x: -1.5,
                                                    y: -25.0,
                                                    width: 34.0,
                                                    height: 5600.0 })));
    }

    #[test]
    fn parsing_invalid_viewboxes_yields_error () {
        assert! (is_parse_error (&ViewBox::from_str ("")));

        assert! (is_value_error (&ViewBox::from_str (" 1,2,-3,-4 ")));

        assert! (is_parse_error (&ViewBox::from_str ("qwerasdfzxcv")));

        assert! (is_parse_error (&ViewBox::from_str (" 1 2 3 4   5")));

        assert! (is_parse_error (&ViewBox::from_str (" 1 2 foo 3 4")));
    }
}
