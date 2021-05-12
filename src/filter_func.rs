use cssparser::Parser;

use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::filter::Filter;
use crate::filters::{
    gaussian_blur::GaussianBlur, FilterResolveError, FilterSpec, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};
use crate::length::*;
use crate::parsers::{NumberOrPercentage, Parse};
use crate::properties::ComputedValues;
use crate::{coord_units::CoordUnits, filters::color_matrix::ColorMatrix};

/// CSS Filter functions from the Filter Effects Module Level 1
///
/// https://www.w3.org/TR/filter-effects/#filter-functions
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFunction {
    Blur(Blur),
    Sepia(Sepia),
}

/// Parameters for the `blur()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-blur
#[derive(Debug, Clone, PartialEq)]
pub struct Blur {
    std_deviation: Option<Length<Both>>,
}

/// Parameters for the `sepia()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-sepia
#[derive(Debug, Clone, PartialEq)]
pub struct Sepia {
    proportion: Option<f64>,
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

// This function doesn't fail, but returns a Result like the other parsers, so tell Clippy
// about that.
#[allow(clippy::unnecessary_wraps)]
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

#[allow(clippy::unnecessary_wraps)]
fn parse_sepia<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = match parser.try_parse(|p| NumberOrPercentage::parse(p)) {
        Ok(NumberOrPercentage { value }) if value < 0.0 => None,
        Ok(NumberOrPercentage { value }) => Some(value.clamp(0.0, 1.0)),
        Err(_) => None,
    };

    Ok(FilterFunction::Sepia(Sepia { proportion }))
}

impl Blur {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        // The 0.0 default is from the spec
        let std_dev = self.std_deviation.map(|l| l.to_user(params)).unwrap_or(0.0);

        let user_space_filter = Filter::default().to_user_space(params);

        let gaussian_blur = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::GaussianBlur(GaussianBlur {
                std_deviation: (std_dev, std_dev),
                ..GaussianBlur::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![gaussian_blur],
        }
    }
}

impl Sepia {
    #[rustfmt::skip]
    fn matrix(&self) -> nalgebra::Matrix5<f64> {
        let p = self.proportion.unwrap_or(1.0);

        nalgebra::Matrix5::new(
            0.393 + 0.607 * (1.0 - p), 0.769 - 0.769 * (1.0 - p), 0.189 - 0.189 * (1.0 - p), 0.0, 0.0,
            0.349 - 0.349 * (1.0 - p), 0.686 + 0.314 * (1.0 - p), 0.168 - 0.168 * (1.0 - p), 0.0, 0.0,
            0.272 - 0.272 * (1.0 - p), 0.534 - 0.534 * (1.0 - p), 0.131 + 0.869 * (1.0 - p), 0.0, 0.0,
            0.0,                       0.0,                       0.0,                       1.0, 0.0,
            0.0,                       0.0,                       0.0,                       0.0, 1.0,
        )
    }

    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let user_space_filter = Filter::default().to_user_space(params);

        let sepia = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ColorMatrix(ColorMatrix {
                matrix: self.matrix(),
                ..ColorMatrix::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![sepia],
        }
    }
}

impl Parse for FilterFunction {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, crate::error::ParseError<'i>> {
        let loc = parser.current_source_location();

        if let Ok(func) = parser.try_parse(|p| parse_function(p, "blur", parse_blur)) {
            return Ok(func);
        }

        if let Ok(func) = parser.try_parse(|p| parse_function(p, "sepia", parse_sepia)) {
            return Ok(func);
        }

        return Err(loc.new_custom_error(ValueErrorKind::parse_error("expected filter function")));
    }
}

impl FilterFunction {
    // If this function starts actually returning an Err, remove this Clippy exception:
    #[allow(clippy::unnecessary_wraps)]
    pub fn to_filter_spec(
        &self,
        values: &ComputedValues,
        draw_ctx: &DrawingCtx,
    ) -> Result<FilterSpec, FilterResolveError> {
        // userSpaceonUse is the default for primitive_units
        let view_params = draw_ctx.push_coord_units(CoordUnits::UserSpaceOnUse);
        let params = NormalizeParams::new(values, &view_params);

        match self {
            FilterFunction::Blur(v) => Ok(v.to_filter_spec(&params)),
            FilterFunction::Sepia(v) => Ok(v.to_filter_spec(&params)),
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
    fn parses_sepia() {
        assert_eq!(
            FilterFunction::parse_str("sepia()").unwrap(),
            FilterFunction::Sepia(Sepia { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("sepia(80%)").unwrap(),
            FilterFunction::Sepia(Sepia {
                proportion: Some(0.80_f32.into())
            })
        );

        assert_eq!(
            FilterFunction::parse_str("sepia(0.52)").unwrap(),
            FilterFunction::Sepia(Sepia {
                proportion: Some(0.52_f32.into())
            })
        );

        // values > 1.0 should be clamped to 1.0
        assert_eq!(
            FilterFunction::parse_str("sepia(1.5)").unwrap(),
            FilterFunction::Sepia(Sepia {
                proportion: Some(1.0)
            })
        );

        // negative numbers are invalid.
        assert_eq!(
            FilterFunction::parse_str("sepia(-1)").unwrap(),
            FilterFunction::Sepia(Sepia { proportion: None }),
        );
    }

    #[test]
    fn invalid_blur_yields_error() {
        assert!(FilterFunction::parse_str("blur(foo)").is_err());
        assert!(FilterFunction::parse_str("blur(42 43)").is_err());
    }

    #[test]
    fn invalid_sepia_yields_error() {
        assert!(FilterFunction::parse_str("sepia(foo)").is_err());
    }
}
