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

impl RsvgViewBox {
    pub fn new (rect: cairo::Rectangle,
                active: bool) -> RsvgViewBox {
        RsvgViewBox {
            rect: rect,
            active: active.to_glib ()
        }
    }

    pub fn new_inactive () -> RsvgViewBox {
        RsvgViewBox::new (cairo::Rectangle { x: 0.0,
                                             y: 0.0,
                                             width: 0.0,
                                             height: 0.0 },
                          false)
    }

    pub fn is_active (&self) -> bool {
        from_glib (self.active)
    }
}

impl Default for RsvgViewBox {
    fn default () -> RsvgViewBox {
        RsvgViewBox::new_inactive ()
    }
}

impl FromStr for RsvgViewBox {
    type Err = AttributeError;

    // Parse a viewBox attribute
    // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
    //
    // viewBox: double [,] double [,] double [,] double [,]
    //
    // x, y, w, h
    //
    // Where w and h must be nonnegative.
    fn from_str (s: &str) -> Result<RsvgViewBox, AttributeError> {
        let v = parsers::number_list (s, ListLength::Exact (4))
            .map_err (|_| ParseError::new ("string does not match 'x [,] y [,] w [,] h'"))?;

        let (x, y, w, h) = (v[0], v[1], v[2], v[3]);

        if w >= 0.0 && h >= 0.0 {
            Ok (RsvgViewBox::new (cairo::Rectangle { x: x,
                                                     y: y,
                                                     width: w,
                                                     height: h },
                                  true))
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
        assert_eq! (RsvgViewBox::from_str ("  1 2 3 4"),
                    Ok (RsvgViewBox::new (cairo::Rectangle { x: 1.0,
                                                             y: 2.0,
                                                             width: 3.0,
                                                             height: 4.0 },
                                          true)));

        assert_eq! (RsvgViewBox::from_str (" -1.5 -2.5e1,34,56e2  "),
                    Ok (RsvgViewBox::new (cairo::Rectangle { x: -1.5,
                                                             y: -25.0,
                                                             width: 34.0,
                                                             height: 5600.0 },
                                          true)));
    }

    #[test]
    fn parsing_invalid_viewboxes_yields_error () {
        assert! (is_parse_error (&RsvgViewBox::from_str ("")));

        assert! (is_value_error (&RsvgViewBox::from_str (" 1,2,-3,-4 ")));

        assert! (is_parse_error (&RsvgViewBox::from_str ("qwerasdfzxcv")));

        assert! (is_parse_error (&RsvgViewBox::from_str (" 1 2 3 4   5")));

        assert! (is_parse_error (&RsvgViewBox::from_str (" 1 2 foo 3 4")));
    }
}
