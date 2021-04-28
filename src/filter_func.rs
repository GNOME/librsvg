use cssparser::Parser;

use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{DrawingCtx, ViewParams};
use crate::error::*;
use crate::filter::Filter;
use crate::filters::{
    gaussian_blur::GaussianBlur, FilterResolveError, FilterSpec, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};
use crate::length::*;
use crate::parsers::Parse;
use crate::properties::ComputedValues;

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

impl Blur {
    fn to_filter_spec(
        &self,
        values: &ComputedValues,
        params: &ViewParams,
    ) -> Result<FilterSpec, FilterResolveError> {
        // The 0.0 default is from the spec
        let std_dev = self
            .std_deviation
            .map(|l| l.normalize(values, params))
            .unwrap_or(0.0);

        let user_space_filter = Filter::default().to_user_space(values, params);

        let gaussian_blur = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::GaussianBlur(GaussianBlur {
                std_deviation: (std_dev, std_dev),
                ..GaussianBlur::default()
            }),
        }
        .into_user_space(values, params);

        Ok(FilterSpec {
            user_space_filter,
            primitives: vec![gaussian_blur],
        })
    }
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
    pub fn to_filter_spec(
        &self,
        values: &ComputedValues,
        draw_ctx: &DrawingCtx,
    ) -> Result<FilterSpec, FilterResolveError> {
        // This is the default for primitive_units
        let params = draw_ctx.push_coord_units(CoordUnits::UserSpaceOnUse);

        match self {
            FilterFunction::Blur(v) => v.to_filter_spec(values, &params),
        }
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
