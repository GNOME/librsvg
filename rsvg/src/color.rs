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

fn parse_plain_color<'i>(parser: &mut Parser<'i, '_>) -> Result<cssparser::Color, ParseError<'i>> {
    let loc = parser.current_source_location();

    let color = cssparser::Color::parse(parser).map_err(map_color_parse_error)?;

    // Return only supported color types, and mark the others as errors.
    match color {
        Color::CurrentColor | Color::Rgba(_) | Color::Hsl(_) | Color::Hwb(_) => Ok(color),

        _ => Err(ParseError {
            kind: ParseErrorKind::Custom(ValueErrorKind::parse_error("unsupported color syntax")),
            location: loc,
        }),
    }
}

/// Parse a custom property name.
///
/// <https://drafts.csswg.org/css-variables/#typedef-custom-property-name>
fn parse_name(s: &str) -> Result<&str, ()> {
    if s.starts_with("--") && s.len() > 2 {
        Ok(&s[2..])
    } else {
        Err(())
    }
}

fn parse_var_with_fallback<'i>(
    parser: &mut Parser<'i, '_>,
) -> Result<cssparser::Color, ParseError<'i>> {
    let name = parser.expect_ident_cloned()?;

    // ignore the name for now; we'll use it later when we actually
    // process the names of custom variables
    let _name = parse_name(&name).map_err(|()| {
        parser.new_custom_error(ValueErrorKind::parse_error(&format!(
            "unexpected identifier {}",
            name
        )))
    })?;

    parser.expect_comma()?;

    // FIXME: when fixing #459 (full support for var()), note that
    // https://drafts.csswg.org/css-variables/#using-variables indicates that var(--a,) is
    // a valid function, which means that the fallback value is an empty set of tokens.
    //
    // Also, see Servo's extra code to handle semicolons and stuff in toplevel rules.
    //
    // Also, tweak the tests tagged with "FIXME: var()" below.

    parse_plain_color(parser)
}

impl Parse for cssparser::Color {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<cssparser::Color, ParseError<'i>> {
        if let Ok(c) = parser.try_parse(|p| {
            p.expect_function_matching("var")?;
            p.parse_nested_block(parse_var_with_fallback)
        }) {
            Ok(c)
        } else {
            parse_plain_color(parser)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_color() {
        assert_eq!(
            Color::parse_str("#112233").unwrap(),
            Color::Rgba(RGBA::new(Some(0x11), Some(0x22), Some(0x33), Some(1.0)))
        );
    }

    #[test]
    fn var_with_fallback_parses_as_color() {
        assert_eq!(
            Color::parse_str("var(--foo, #112233)").unwrap(),
            Color::Rgba(RGBA::new(Some(0x11), Some(0x22), Some(0x33), Some(1.0)))
        );

        assert_eq!(
            Color::parse_str("var(--foo, rgb(100% 50% 25%)").unwrap(),
            Color::Rgba(RGBA::new(Some(0xff), Some(0x80), Some(0x40), Some(1.0)))
        );
    }

    // FIXME: var() - when fixing #459, see the note in the code above.  All the syntaxes
    // in this test function will become valid once we have full support for var().
    #[test]
    fn var_without_fallback_yields_error() {
        assert!(Color::parse_str("var(--foo)").is_err());
        assert!(Color::parse_str("var(--foo,)").is_err());
        assert!(Color::parse_str("var(--foo, )").is_err());
        assert!(Color::parse_str("var(--foo, this is not a color)").is_err());
        assert!(Color::parse_str("var(--foo, #112233, blah)").is_err());
    }
}
