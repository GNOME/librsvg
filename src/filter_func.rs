use cssparser::Parser;

use crate::error::*;
use crate::filters::{FilterResolveError, FilterSpec};
use crate::length::*;
use crate::parsers::Parse;

/// CSS Filter functions from the Filter Effects Module Level 1
///
/// https://www.w3.org/TR/filter-effects/#filter-functions
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFunction {
    Blur(Blur),
}

/// Parameters for the `blur()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-blur
#[derive(Debug, Clone, PartialEq)]
pub struct Blur {
    std_deviation: Option<Length<Both>>,
}

fn parse_function<'i, F>(
    parser: &mut Parser<'i, '_>,
    name: &str,
    f: F,
) -> Result<FilterFunction, ParseError<'i>>
where
    F: for<'tt> FnOnce(&mut Parser<'i, 'tt>) -> Result<FilterFunction, ParseError<'i>>,
{
    parser.expect_function_matching(name)?;
    parser.parse_nested_block(f)
}

fn parse_blur<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let length = if let Ok(length) = parser.try_parse(|p| Length::parse(p)) {
        Some(length)
    } else {
        None
    };

    Ok(FilterFunction::Blur(Blur {
        std_deviation: length,
    }))
}

impl Parse for FilterFunction {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, crate::error::ParseError<'i>> {
        let loc = parser.current_source_location();

        parser
            .try_parse(|p| parse_function(p, "blur", parse_blur))
            .or_else(|_| {
                Err(loc.new_custom_error(ValueErrorKind::parse_error("expected filter function")))
            })
    }
}

impl FilterFunction {
    pub fn to_filter_spec(&self) -> Result<FilterSpec, FilterResolveError> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blur() {
        assert_eq!(
            FilterFunction::parse_str("blur()").unwrap(),
            FilterFunction::Blur(Blur {
                std_deviation: None
            })
        );

        assert_eq!(
            FilterFunction::parse_str("blur(5px)").unwrap(),
            FilterFunction::Blur(Blur {
                std_deviation: Some(Length::new(5.0, LengthUnit::Px))
            })
        );
    }

    #[test]
    fn invalid_blur_yields_error() {
        assert!(FilterFunction::parse_str("blur(foo)").is_err());
        assert!(FilterFunction::parse_str("blur(42 43)").is_err());
    }
}
