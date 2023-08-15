//! CSS color values.

use cssparser::{hsl_to_rgb, hwb_to_rgb, Color, ParseErrorKind, Parser, RGBA};

use crate::error::*;
use crate::parsers::Parse;

/// Turn a short-lived [`cssparser::ParseError`] into a long-lived [`ParseError`].
///
/// cssparser's error type has a lifetime equal to the string being parsed.  We want
/// a long-lived error so we can store it away if needed.  Basically, here we turn
/// a `&str` into a `String`.
fn map_color_parse_error(err: cssparser::ParseError<'_, ()>) -> ParseError<'_> {
    let string_err = match err.kind {
        ParseErrorKind::Basic(ref e) => format!("{}", e),
        ParseErrorKind::Custom(()) => {
            // In cssparser 0.31, the error type for Color::parse is defined like this:
            //
            //   pub fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Color, ParseError<'i, ()>> {
            //
            // The ParseError<'i, ()> means that the ParseErrorKind::Custom(T) variant will have
            // T be the () type.
            //
            // So, here we match for () inside the Custom variant.  If cssparser
            // changes its error API, this match will hopefully catch errors.
            //
            // Implementation detail: Color::parse() does not ever return Custom errors, only
            // Basic ones.  So the match for Basic above handles everything, and this one
            // for () is a dummy case.
            "could not parse color".to_string()
        }
    };

    ParseError {
        kind: ParseErrorKind::Custom(ValueErrorKind::Parse(string_err)),
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

pub fn color_to_rgba(color: &Color) -> RGBA {
    match color {
        Color::Rgba(rgba) => *rgba,

        Color::Hsl(hsl) => {
            let (red, green, blue) = hsl_to_rgb(
                hsl.hue.unwrap_or(0.0) / 360.0,
                hsl.saturation.unwrap_or(0.0),
                hsl.lightness.unwrap_or(0.0),
            );

            RGBA::from_floats(Some(red), Some(green), Some(blue), hsl.alpha)
        }

        Color::Hwb(hwb) => {
            let (red, green, blue) = hwb_to_rgb(
                hwb.hue.unwrap_or(0.0) / 360.0,
                hwb.whiteness.unwrap_or(0.0),
                hwb.blackness.unwrap_or(0.0),
            );

            RGBA::from_floats(Some(red), Some(green), Some(blue), hwb.alpha)
        }

        _ => unimplemented!(),
    }
}
