//! CSS color values.

use cssparser::Parser;

use crate::error::*;
use crate::parsers::Parse;

pub use cssparser::Color;

impl Parse for cssparser::Color {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<cssparser::Color, ParseError<'i>> {
        Ok(cssparser::Color::parse(parser)?)
    }
}

impl Parse for cssparser::RGBA {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<cssparser::RGBA, ParseError<'i>> {
        let loc = parser.current_source_location();

        match cssparser::Color::parse(parser)? {
            cssparser::Color::RGBA(rgba) => Ok(rgba),
            cssparser::Color::CurrentColor => Err(loc.new_custom_error(ValueErrorKind::Value(
                "currentColor is not allowed here".to_string(),
            ))),
        }
    }
}
