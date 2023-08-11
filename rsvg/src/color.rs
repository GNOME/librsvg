//! CSS color values.

use cssparser::{ParseErrorKind, Parser};

use crate::error::*;
use crate::parsers::Parse;

pub use cssparser::Color;

fn map_color_parse_error(err: cssparser::ParseError<'_, ()>) -> ParseError<'_> {
    ParseError {
        kind: ParseErrorKind::Custom(ValueErrorKind::parse_error("Could not parse color")),
        location: err.location,
    }
}

impl Parse for cssparser::Color {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<cssparser::Color, ParseError<'i>> {
        cssparser::Color::parse(parser).map_err(map_color_parse_error)
    }
}

impl Parse for cssparser::RGBA {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<cssparser::RGBA, ParseError<'i>> {
        let loc = parser.current_source_location();

        match cssparser::Color::parse(parser).map_err(map_color_parse_error)? {
            cssparser::Color::Rgba(rgba) => Ok(rgba),
            cssparser::Color::CurrentColor => Err(loc.new_custom_error(ValueErrorKind::Value(
                "currentColor is not allowed here".to_string(),
            ))),
            _ => Err(loc.new_custom_error(ValueErrorKind::value_error("Unsupported color type"))),
        }
    }
}
