use cssparser::Parser;

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
use crate::{drawing_ctx::DrawingCtx, filters::component_transfer};

/// CSS Filter functions from the Filter Effects Module Level 1
///
/// https://www.w3.org/TR/filter-effects/#filter-functions
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFunction {
    Blur(Blur),
    Brightness(Brightness),
    Contrast(Contrast),
    Grayscale(Grayscale),
    Invert(Invert),
    Opacity(Opacity),
    Saturate(Saturate),
    Sepia(Sepia),
}

/// Parameters for the `blur()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-blur
#[derive(Debug, Clone, PartialEq)]
pub struct Blur {
    std_deviation: Option<Length<Both>>,
}

/// Parameters for the `brightness()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-brightness
#[derive(Debug, Clone, PartialEq)]
pub struct Brightness {
    proportion: Option<f64>,
}

/// Parameters for the `contrast()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-contrast
#[derive(Debug, Clone, PartialEq)]
pub struct Contrast {
    proportion: Option<f64>,
}

/// Parameters for the `grayscale()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-grayscale
#[derive(Debug, Clone, PartialEq)]
pub struct Grayscale {
    proportion: Option<f64>,
}

/// Parameters for the `invert()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-invert
#[derive(Debug, Clone, PartialEq)]
pub struct Invert {
    proportion: Option<f64>,
}

/// Parameters for the `opacity()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-opacity
#[derive(Debug, Clone, PartialEq)]
pub struct Opacity {
    proportion: Option<f64>,
}

/// Parameters for the `saturate()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-saturate
#[derive(Debug, Clone, PartialEq)]
pub struct Saturate {
    proportion: Option<f64>,
}

/// Parameters for the `sepia()` filter function
///
/// https://www.w3.org/TR/filter-effects/#funcdef-filter-sepia
#[derive(Debug, Clone, PartialEq)]
pub struct Sepia {
    proportion: Option<f64>,
}

/// Reads an optional number or percentage from the parser.
/// Negative numbers are not allowed.
fn parse_num_or_percentage<'i>(parser: &mut Parser<'i, '_>) -> Option<f64> {
    match parser.try_parse(|p| NumberOrPercentage::parse(p)) {
        Ok(NumberOrPercentage { value }) if value < 0.0 => None,
        Ok(NumberOrPercentage { value }) => Some(value),
        Err(_) => None,
    }
}

/// Reads an optional number or percentage from the parser, returning a value clamped to [0, 1].
/// Negative numbers are not allowed.
fn parse_num_or_percentage_clamped<'i>(parser: &mut Parser<'i, '_>) -> Option<f64> {
    parse_num_or_percentage(parser).map(|value| value.clamp(0.0, 1.0))
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
    let length = parser.try_parse(|p| Length::parse(p)).ok();

    Ok(FilterFunction::Blur(Blur {
        std_deviation: length,
    }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_brightness<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage(parser);

    Ok(FilterFunction::Brightness(Brightness { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_contrast<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage(parser);

    Ok(FilterFunction::Contrast(Contrast { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_grayscale<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage_clamped(parser);

    Ok(FilterFunction::Grayscale(Grayscale { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_invert<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage_clamped(parser);

    Ok(FilterFunction::Invert(Invert { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_opacity<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage_clamped(parser);

    Ok(FilterFunction::Opacity(Opacity { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_saturate<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage(parser);

    Ok(FilterFunction::Saturate(Saturate { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_sepia<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage_clamped(parser);

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

impl Brightness {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let user_space_filter = Filter::default().to_user_space(params);
        let slope = self.proportion.unwrap_or(1.0);

        let brightness = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ComponentTransfer(component_transfer::ComponentTransfer {
                functions: component_transfer::Functions {
                    r: component_transfer::FeFuncR {
                        function_type: component_transfer::FunctionType::Linear,
                        slope,
                        ..component_transfer::FeFuncR::default()
                    },
                    g: component_transfer::FeFuncG {
                        function_type: component_transfer::FunctionType::Linear,
                        slope,
                        ..component_transfer::FeFuncG::default()
                    },
                    b: component_transfer::FeFuncB {
                        function_type: component_transfer::FunctionType::Linear,
                        slope,
                        ..component_transfer::FeFuncB::default()
                    },
                    ..component_transfer::Functions::default()
                },
                ..component_transfer::ComponentTransfer::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![brightness],
        }
    }
}

impl Contrast {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let user_space_filter = Filter::default().to_user_space(params);
        let slope = self.proportion.unwrap_or(1.0);
        let intercept = -(0.5 * slope) + 0.5;

        let contrast = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ComponentTransfer(component_transfer::ComponentTransfer {
                functions: component_transfer::Functions {
                    r: component_transfer::FeFuncR {
                        function_type: component_transfer::FunctionType::Linear,
                        slope,
                        intercept,
                        ..component_transfer::FeFuncR::default()
                    },
                    g: component_transfer::FeFuncG {
                        function_type: component_transfer::FunctionType::Linear,
                        slope,
                        intercept,
                        ..component_transfer::FeFuncG::default()
                    },
                    b: component_transfer::FeFuncB {
                        function_type: component_transfer::FunctionType::Linear,
                        slope,
                        intercept,
                        ..component_transfer::FeFuncB::default()
                    },
                    ..component_transfer::Functions::default()
                },
                ..component_transfer::ComponentTransfer::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![contrast],
        }
    }
}

impl Grayscale {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        // grayscale is implemented as the inverse of a saturate operation,
        // with the input clamped to the range [0, 1] by the parser.
        let p = 1.0 - self.proportion.unwrap_or(1.0);
        let saturate = Saturate {
            proportion: Some(p),
        };

        saturate.to_filter_spec(params)
    }
}

impl Invert {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let p = self.proportion.unwrap_or(1.0);
        let user_space_filter = Filter::default().to_user_space(params);

        let invert = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ComponentTransfer(component_transfer::ComponentTransfer {
                functions: component_transfer::Functions {
                    r: component_transfer::FeFuncR {
                        function_type: component_transfer::FunctionType::Table,
                        table_values: vec![p, 1.0 - p],
                        ..component_transfer::FeFuncR::default()
                    },
                    g: component_transfer::FeFuncG {
                        function_type: component_transfer::FunctionType::Table,
                        table_values: vec![p, 1.0 - p],
                        ..component_transfer::FeFuncG::default()
                    },
                    b: component_transfer::FeFuncB {
                        function_type: component_transfer::FunctionType::Table,
                        table_values: vec![p, 1.0 - p],
                        ..component_transfer::FeFuncB::default()
                    },
                    ..component_transfer::Functions::default()
                },
                ..component_transfer::ComponentTransfer::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![invert],
        }
    }
}

impl Opacity {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let p = self.proportion.unwrap_or(1.0);
        let user_space_filter = Filter::default().to_user_space(params);

        let opacity = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ComponentTransfer(component_transfer::ComponentTransfer {
                functions: component_transfer::Functions {
                    a: component_transfer::FeFuncA {
                        function_type: component_transfer::FunctionType::Table,
                        table_values: vec![0.0, p],
                        ..component_transfer::FeFuncA::default()
                    },
                    ..component_transfer::Functions::default()
                },
                ..component_transfer::ComponentTransfer::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![opacity],
        }
    }
}

impl Saturate {
    #[rustfmt::skip]
    fn matrix(&self) -> nalgebra::Matrix5<f64> {
        let p = self.proportion.unwrap_or(1.0);

        nalgebra::Matrix5::new(
            0.213 + 0.787 * p, 0.715 - 0.715 * p, 0.072 - 0.072 * p, 0.0, 0.0,
            0.213 - 0.213 * p, 0.715 + 0.285 * p, 0.072 - 0.072 * p, 0.0, 0.0,
            0.213 - 0.213 * p, 0.715 - 0.715 * p, 0.072 + 0.928 * p, 0.0, 0.0,
            0.0,               0.0,               0.0,               1.0, 0.0,
            0.0,               0.0,               0.0,               0.0, 1.0,
        )
    }

    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let user_space_filter = Filter::default().to_user_space(params);

        let saturate = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ColorMatrix(ColorMatrix {
                matrix: self.matrix(),
                ..ColorMatrix::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![saturate],
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
        let fns: Vec<(&str, &dyn Fn(&mut Parser<'i, '_>) -> _)> = vec![
            ("blur", &parse_blur),
            ("brightness", &parse_brightness),
            ("contrast", &parse_contrast),
            ("grayscale", &parse_grayscale),
            ("invert", &parse_invert),
            ("opacity", &parse_opacity),
            ("saturate", &parse_saturate),
            ("sepia", &parse_sepia),
        ];

        for (filter_name, parse_fn) in fns {
            if let Ok(func) = parser.try_parse(|p| parse_function(p, filter_name, parse_fn)) {
                return Ok(func);
            }
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
            FilterFunction::Brightness(v) => Ok(v.to_filter_spec(&params)),
            FilterFunction::Contrast(v) => Ok(v.to_filter_spec(&params)),
            FilterFunction::Grayscale(v) => Ok(v.to_filter_spec(&params)),
            FilterFunction::Invert(v) => Ok(v.to_filter_spec(&params)),
            FilterFunction::Opacity(v) => Ok(v.to_filter_spec(&params)),
            FilterFunction::Saturate(v) => Ok(v.to_filter_spec(&params)),
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
    fn parses_brightness() {
        assert_eq!(
            FilterFunction::parse_str("brightness()").unwrap(),
            FilterFunction::Brightness(Brightness { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("brightness(50%)").unwrap(),
            FilterFunction::Brightness(Brightness {
                proportion: Some(0.50_f32.into()),
            })
        );
    }

    #[test]
    fn parses_contrast() {
        assert_eq!(
            FilterFunction::parse_str("contrast()").unwrap(),
            FilterFunction::Contrast(Contrast { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("contrast(50%)").unwrap(),
            FilterFunction::Contrast(Contrast {
                proportion: Some(0.50_f32.into()),
            })
        );
    }

    #[test]
    fn parses_grayscale() {
        assert_eq!(
            FilterFunction::parse_str("grayscale()").unwrap(),
            FilterFunction::Grayscale(Grayscale { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("grayscale(50%)").unwrap(),
            FilterFunction::Grayscale(Grayscale {
                proportion: Some(0.50_f32.into()),
            })
        );
    }

    #[test]
    fn parses_invert() {
        assert_eq!(
            FilterFunction::parse_str("invert()").unwrap(),
            FilterFunction::Invert(Invert { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("invert(50%)").unwrap(),
            FilterFunction::Invert(Invert {
                proportion: Some(0.50_f32.into()),
            })
        );
    }

    #[test]
    fn parses_opacity() {
        assert_eq!(
            FilterFunction::parse_str("opacity()").unwrap(),
            FilterFunction::Opacity(Opacity { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("opacity(50%)").unwrap(),
            FilterFunction::Opacity(Opacity {
                proportion: Some(0.50_f32.into()),
            })
        );
    }

    #[test]
    fn parses_saturate() {
        assert_eq!(
            FilterFunction::parse_str("saturate()").unwrap(),
            FilterFunction::Saturate(Saturate { proportion: None })
        );

        assert_eq!(
            FilterFunction::parse_str("saturate(50%)").unwrap(),
            FilterFunction::Saturate(Saturate {
                proportion: Some(0.50_f32.into()),
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
    fn invalid_brightness_yields_error() {
        assert!(FilterFunction::parse_str("brightness(foo)").is_err());
    }

    #[test]
    fn invalid_contrast_yields_error() {
        assert!(FilterFunction::parse_str("contrast(foo)").is_err());
    }

    #[test]
    fn invalid_grayscale_yields_error() {
        assert!(FilterFunction::parse_str("grayscale(foo)").is_err());
    }

    #[test]
    fn invalid_invert_yields_error() {
        assert!(FilterFunction::parse_str("invert(foo)").is_err());
    }

    #[test]
    fn invalid_opacity_yields_error() {
        assert!(FilterFunction::parse_str("opacity(foo)").is_err());
    }

    #[test]
    fn invalid_saturate_yields_error() {
        assert!(FilterFunction::parse_str("saturate(foo)").is_err());
    }

    #[test]
    fn invalid_sepia_yields_error() {
        assert!(FilterFunction::parse_str("sepia(foo)").is_err());
    }
}
