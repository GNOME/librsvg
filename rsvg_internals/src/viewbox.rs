//! Parser for the `viewBox` attribute.

use cssparser::Parser;

use crate::error::*;
use crate::number_list::{NumberList, NumberListLength};
use crate::parsers::Parse;
use crate::rect::Rect;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ViewBox(pub Rect);

impl Parse for ViewBox {
    // Parse a viewBox attribute
    // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
    //
    // viewBox: double [,] double [,] double [,] double [,]
    //
    // x, y, w, h
    //
    // Where w and h must be nonnegative.
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<ViewBox, ParseError<'i>> {
        let loc = parser.current_source_location();

        let NumberList(v) = NumberList::parse(parser, NumberListLength::Exact(4))?;

        let (x, y, width, height) = (v[0], v[1], v[2], v[3]);

        if width >= 0.0 && height >= 0.0 {
            Ok(ViewBox(Rect::new(x, y, x + width, y + height)))
        } else {
            Err(loc.new_custom_error(ValueErrorKind::value_error(
                "width and height must not be negative",
            )))
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
            Ok(ViewBox(Rect::new(1.0, 2.0, 4.0, 6.0)))
        );

        assert_eq!(
            ViewBox::parse_str(" -1.5 -2.5e1,34,56e2  "),
            Ok(ViewBox(Rect::new(-1.5, -25.0, 32.5, 5575.0)))
        );
    }

    #[test]
    fn parsing_invalid_viewboxes_yields_error() {
        assert!(ViewBox::parse_str("").is_err());
        assert!(ViewBox::parse_str(" 1,2,-3,-4 ").is_err());
        assert!(ViewBox::parse_str("qwerasdfzxcv").is_err());
        assert!(ViewBox::parse_str(" 1 2 3 4   5").is_err());
        assert!(ViewBox::parse_str(" 1 2 foo 3 4").is_err());

        // https://gitlab.gnome.org/GNOME/librsvg/issues/344
        assert!(ViewBox::parse_str("0 0 9E80.7").is_err());
    }
}
