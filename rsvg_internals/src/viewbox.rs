use cssparser::Parser;

use crate::error::*;
use crate::number_list::{NumberList, NumberListLength};
use crate::parsers::{Parse, ParseError};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ViewBox {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> ViewBox {
        assert!(
            w >= 0.0 && h >= 0.0,
            "width and height must not be negative"
        );

        ViewBox {
            x,
            y,
            width: w,
            height: h,
        }
    }
}

impl Parse for ViewBox {
    type Err = ValueErrorKind;

    // Parse a viewBox attribute
    // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
    //
    // viewBox: double [,] double [,] double [,] double [,]
    //
    // x, y, w, h
    //
    // Where w and h must be nonnegative.
    fn parse(parser: &mut Parser<'_, '_>) -> Result<ViewBox, ValueErrorKind> {
        let NumberList(v) = NumberList::parse(parser, NumberListLength::Exact(4))
            .map_err(|_| ParseError::new("string does not match 'x [,] y [,] w [,] h'"))?;

        let (x, y, width, height) = (v[0], v[1], v[2], v[3]);

        if width >= 0.0 && height >= 0.0 {
            Ok(ViewBox {
                x,
                y,
                width,
                height,
            })
        } else {
            Err(ValueErrorKind::Value(
                "width and height must not be negative".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_viewboxes() {
        assert_eq!(
            ViewBox::parse_str("  1 2 3 4"),
            Ok(ViewBox::new(1.0, 2.0, 3.0, 4.0))
        );

        assert_eq!(
            ViewBox::parse_str(" -1.5 -2.5e1,34,56e2  "),
            Ok(ViewBox::new(-1.5, -25.0, 34.0, 5600.0))
        );
    }

    #[test]
    fn parsing_invalid_viewboxes_yields_error() {
        assert!(is_parse_error(&ViewBox::parse_str("")));

        assert!(is_value_error(&ViewBox::parse_str(" 1,2,-3,-4 ")));

        assert!(is_parse_error(&ViewBox::parse_str("qwerasdfzxcv")));

        assert!(is_parse_error(&ViewBox::parse_str(" 1 2 3 4   5")));

        assert!(is_parse_error(&ViewBox::parse_str(" 1 2 foo 3 4")));

        // https://gitlab.gnome.org/GNOME/librsvg/issues/344
        assert!(is_parse_error(&ViewBox::parse_str("0 0 9E80.7")));
    }
}
