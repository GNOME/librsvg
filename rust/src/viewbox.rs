extern crate cairo;

use std::str::FromStr;

use error::*;
use parsers::ParseError;
use parsers;

/* Keep this in sync with rsvg-private.h:RsvgViewBox */
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RsvgViewBox {
    pub rect:   cairo::Rectangle,
    pub active: bool
}

impl RsvgViewBox {
    pub fn new_inactive () -> RsvgViewBox {
        RsvgViewBox {
            rect: cairo::Rectangle { x: 0.0,
                                     y: 0.0,
                                     width: 0.0,
                                     height: 0.0 },
            active: false
        }
    }
}

impl Default for RsvgViewBox {
    fn default () -> RsvgViewBox {
        RsvgViewBox::new_inactive ()
    }
}

impl FromStr for RsvgViewBox {
    type Err = AttributeError;

    fn from_str (s: &str) -> Result<RsvgViewBox, AttributeError> {
        let result = parsers::view_box (s.trim ().as_bytes ()).to_full_result ();

        match result {
            Ok ((x, y, w, h)) => {
                if w >= 0.0 && h >= 0.0 {
                    Ok (RsvgViewBox { rect: cairo::Rectangle { x: x,
                                                               y: y,
                                                               width: w,
                                                               height: h },
                                      active: true })
                } else {
                    Err (AttributeError::Value ("width and height must not be negative".to_string ()))
                }
            },

            Err (_) => {
                Err (AttributeError::Parse (ParseError::new ("string does not match 'x [,] y [,] w [,] h'")))
            }
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
                    Ok (RsvgViewBox { rect: cairo::Rectangle { x: 1.0,
                                                               y: 2.0,
                                                               width: 3.0,
                                                               height: 4.0 },
                                      active: true }));

        assert_eq! (RsvgViewBox::from_str (" -1.5 -2.5e1,34,56e2  "),
                    Ok (RsvgViewBox { rect: cairo::Rectangle { x: -1.5,
                                                               y: -25.0,
                                                               width: 34.0,
                                                               height: 5600.0 },
                                      active: true }));
    }

    #[test]
    fn parsing_invalid_viewboxes_yields_error () {
        assert! (is_parse_error (&RsvgViewBox::from_str ("")));

        assert! (is_value_error (&RsvgViewBox::from_str (" 1,2,-3,-4 ")));

        assert! (is_parse_error (&RsvgViewBox::from_str ("qwerasdfzxcv")));

        assert! (is_parse_error (&RsvgViewBox::from_str (" 1 2 3 4   5")));
    }
}
