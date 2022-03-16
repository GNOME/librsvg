//! SVG2 filter function shortcuts - `blur()`, `brightness()`, etc.
//!
//! The `<filter>` element from SVG1.1 (also present in SVG2) uses some verbose XML to
//! define chains of filter primitives.  In SVG2, there is a shortcut form of the `filter`
//! attribute and property, where one can simply say `filter="blur(5)"` and get the
//! equivalent of writing a full `<filter>` with a `<feGaussianBlur>` element.
//!
//! This module has a type for each of the filter functions in SVG2 with the function's
//! parameters, for example [`Blur`] stores the blur's standard deviation parameter.
//!
//! Those types get aggregated in the [`FilterFunction`] enum.  A [`FilterFunction`] can
//! then convert itself into a [`FilterSpec`], which is ready to be rendered on a surface.

use cssparser::{Color, Parser, RGBA};

use crate::angle::Angle;
use crate::error::*;
use crate::filter::Filter;
use crate::filters::{
    color_matrix::ColorMatrix,
    component_transfer,
    composite::{Composite, Operator},
    flood::Flood,
    gaussian_blur::GaussianBlur,
    merge::{Merge, MergeNode},
    offset::Offset,
    FilterResolveError, FilterSpec, Input, Primitive, PrimitiveParams, ResolvedPrimitive,
};
use crate::length::*;
use crate::paint_server::resolve_color;
use crate::parsers::{CustomIdent, NumberOrPercentage, Parse};
use crate::unit_interval::UnitInterval;

/// CSS Filter functions from the Filter Effects Module Level 1
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#filter-functions>
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFunction {
    Blur(Blur),
    Brightness(Brightness),
    Contrast(Contrast),
    DropShadow(DropShadow),
    Grayscale(Grayscale),
    HueRotate(HueRotate),
    Invert(Invert),
    Opacity(Opacity),
    Saturate(Saturate),
    Sepia(Sepia),
}

/// Parameters for the `blur()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-blur>
#[derive(Debug, Clone, PartialEq)]
pub struct Blur {
    std_deviation: Option<Length<Both>>,
}

/// Parameters for the `brightness()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-brightness>
#[derive(Debug, Clone, PartialEq)]
pub struct Brightness {
    proportion: Option<f64>,
}

/// Parameters for the `contrast()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-contrast>
#[derive(Debug, Clone, PartialEq)]
pub struct Contrast {
    proportion: Option<f64>,
}

/// Parameters for the `drop-shadow()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-drop-shadow>
#[derive(Debug, Clone, PartialEq)]
pub struct DropShadow {
    color: Option<Color>,
    dx: Option<Length<Horizontal>>,
    dy: Option<Length<Vertical>>,
    std_deviation: Option<ULength<Both>>,
}

/// Parameters for the `grayscale()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-grayscale>
#[derive(Debug, Clone, PartialEq)]
pub struct Grayscale {
    proportion: Option<f64>,
}

/// Parameters for the `hue-rotate()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-huerotate>
#[derive(Debug, Clone, PartialEq)]
pub struct HueRotate {
    angle: Option<Angle>,
}

/// Parameters for the `invert()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-invert>
#[derive(Debug, Clone, PartialEq)]
pub struct Invert {
    proportion: Option<f64>,
}

/// Parameters for the `opacity()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-opacity>
#[derive(Debug, Clone, PartialEq)]
pub struct Opacity {
    proportion: Option<f64>,
}

/// Parameters for the `saturate()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-saturate>
#[derive(Debug, Clone, PartialEq)]
pub struct Saturate {
    proportion: Option<f64>,
}

/// Parameters for the `sepia()` filter function
///
/// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#funcdef-filter-sepia>
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
fn parse_dropshadow<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let mut result = DropShadow {
        color: None,
        dx: None,
        dy: None,
        std_deviation: None,
    };

    result.color = parser.try_parse(Color::parse).ok();

    // if dx is provided, dy must follow and an optional std_dev must follow that.
    if let Ok(dx) = parser.try_parse(Length::parse) {
        result.dx = Some(dx);
        result.dy = Some(parser.try_parse(Length::parse)?);
        result.std_deviation = parser.try_parse(ULength::parse).ok();
    }

    let loc = parser.current_source_location();

    // because the color and length arguments can be provided in either order,
    // check again after potentially parsing lengths if the color is now provided.
    // if a color is provided both before and after, that is an error.
    if let Ok(c) = parser.try_parse(Color::parse) {
        if result.color.is_some() {
            return Err(
                loc.new_custom_error(ValueErrorKind::Value("color already specified".to_string()))
            );
        } else {
            result.color = Some(c);
        }
    }

    Ok(FilterFunction::DropShadow(result))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_grayscale<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let proportion = parse_num_or_percentage_clamped(parser);

    Ok(FilterFunction::Grayscale(Grayscale { proportion }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_huerotate<'i>(parser: &mut Parser<'i, '_>) -> Result<FilterFunction, ParseError<'i>> {
    let angle = parser.try_parse(|p| Angle::parse(p)).ok();

    Ok(FilterFunction::HueRotate(HueRotate { angle }))
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

impl DropShadow {
    /// Converts a DropShadow into the set of filter element primitives.
    ///
    /// See <https://www.w3.org/TR/filter-effects/#dropshadowEquivalent>.
    fn to_filter_spec(&self, params: &NormalizeParams, default_color: RGBA) -> FilterSpec {
        let user_space_filter = Filter::default().to_user_space(params);
        let dx = self.dx.map(|l| l.to_user(params)).unwrap_or(0.0);
        let dy = self.dy.map(|l| l.to_user(params)).unwrap_or(0.0);
        let std_dev = self.std_deviation.map(|l| l.to_user(params)).unwrap_or(0.0);
        let color = self
            .color
            .as_ref()
            .map(|c| resolve_color(c, UnitInterval::clamp(1.0), default_color))
            .unwrap_or(default_color);

        let offsetblur = CustomIdent("offsetblur".to_string());

        let gaussian_blur = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::GaussianBlur(GaussianBlur {
                in1: Input::SourceAlpha,
                std_deviation: (std_dev, std_dev),
                ..GaussianBlur::default()
            }),
        }
        .into_user_space(params);

        let offset = ResolvedPrimitive {
            primitive: Primitive {
                result: Some(offsetblur.clone()),
                ..Primitive::default()
            },
            params: PrimitiveParams::Offset(Offset {
                in1: Input::default(),
                dx,
                dy,
            }),
        }
        .into_user_space(params);

        let flood = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::Flood(Flood { color }),
        }
        .into_user_space(params);

        let composite = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::Composite(Composite {
                in2: Input::FilterOutput(offsetblur),
                operator: Operator::In,
                ..Composite::default()
            }),
        }
        .into_user_space(params);

        let merge = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::Merge(Merge {
                merge_nodes: vec![
                    MergeNode::default(),
                    MergeNode {
                        in1: Input::SourceGraphic,
                        ..MergeNode::default()
                    },
                ],
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![gaussian_blur, offset, flood, composite, merge],
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

impl HueRotate {
    fn to_filter_spec(&self, params: &NormalizeParams) -> FilterSpec {
        let rads = self.angle.map(|a| a.radians()).unwrap_or(0.0);
        let user_space_filter = Filter::default().to_user_space(params);

        let huerotate = ResolvedPrimitive {
            primitive: Primitive::default(),
            params: PrimitiveParams::ColorMatrix(ColorMatrix {
                matrix: ColorMatrix::hue_rotate_matrix(rads),
                ..ColorMatrix::default()
            }),
        }
        .into_user_space(params);

        FilterSpec {
            user_space_filter,
            primitives: vec![huerotate],
        }
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
            ("drop-shadow", &parse_dropshadow),
            ("grayscale", &parse_grayscale),
            ("hue-rotate", &parse_huerotate),
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
        params: &NormalizeParams,
        current_color: RGBA,
    ) -> Result<FilterSpec, FilterResolveError> {
        match self {
            FilterFunction::Blur(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::Brightness(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::Contrast(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::DropShadow(v) => Ok(v.to_filter_spec(params, current_color)),
            FilterFunction::Grayscale(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::HueRotate(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::Invert(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::Opacity(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::Saturate(v) => Ok(v.to_filter_spec(params)),
            FilterFunction::Sepia(v) => Ok(v.to_filter_spec(params)),
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
    fn parses_dropshadow() {
        assert_eq!(
            FilterFunction::parse_str("drop-shadow(4px 5px)").unwrap(),
            FilterFunction::DropShadow(DropShadow {
                color: None,
                dx: Some(Length::new(4.0, LengthUnit::Px)),
                dy: Some(Length::new(5.0, LengthUnit::Px)),
                std_deviation: None,
            })
        );

        assert_eq!(
            FilterFunction::parse_str("drop-shadow(#ff0000 4px 5px 32px)").unwrap(),
            FilterFunction::DropShadow(DropShadow {
                color: Some(Color::RGBA(RGBA {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 255
                })),
                dx: Some(Length::new(4.0, LengthUnit::Px)),
                dy: Some(Length::new(5.0, LengthUnit::Px)),
                std_deviation: Some(ULength::new(32.0, LengthUnit::Px)),
            })
        );

        assert_eq!(
            FilterFunction::parse_str("drop-shadow(1px 2px blue)").unwrap(),
            FilterFunction::DropShadow(DropShadow {
                color: Some(Color::RGBA(RGBA {
                    red: 0,
                    green: 0,
                    blue: 255,
                    alpha: 255
                })),
                dx: Some(Length::new(1.0, LengthUnit::Px)),
                dy: Some(Length::new(2.0, LengthUnit::Px)),
                std_deviation: None,
            })
        );

        assert_eq!(
            FilterFunction::parse_str("drop-shadow(1px 2px 3px currentColor)").unwrap(),
            FilterFunction::DropShadow(DropShadow {
                color: Some(Color::CurrentColor),
                dx: Some(Length::new(1.0, LengthUnit::Px)),
                dy: Some(Length::new(2.0, LengthUnit::Px)),
                std_deviation: Some(ULength::new(3.0, LengthUnit::Px)),
            })
        );

        assert_eq!(
            FilterFunction::parse_str("drop-shadow(1 2 3)").unwrap(),
            FilterFunction::DropShadow(DropShadow {
                color: None,
                dx: Some(Length::new(1.0, LengthUnit::Px)),
                dy: Some(Length::new(2.0, LengthUnit::Px)),
                std_deviation: Some(ULength::new(3.0, LengthUnit::Px)),
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
    fn parses_huerotate() {
        assert_eq!(
            FilterFunction::parse_str("hue-rotate()").unwrap(),
            FilterFunction::HueRotate(HueRotate { angle: None })
        );

        assert_eq!(
            FilterFunction::parse_str("hue-rotate(0)").unwrap(),
            FilterFunction::HueRotate(HueRotate {
                angle: Some(Angle::new(0.0))
            })
        );

        assert_eq!(
            FilterFunction::parse_str("hue-rotate(128deg)").unwrap(),
            FilterFunction::HueRotate(HueRotate {
                angle: Some(Angle::from_degrees(128.0))
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
    fn invalid_dropshadow_yields_error() {
        assert!(FilterFunction::parse_str("drop-shadow(blue 5px green)").is_err());
        assert!(FilterFunction::parse_str("drop-shadow(blue 5px 5px green)").is_err());
        assert!(FilterFunction::parse_str("drop-shadow(blue 1px)").is_err());
        assert!(FilterFunction::parse_str("drop-shadow(1 2 3 4 blue)").is_err());
    }

    #[test]
    fn invalid_grayscale_yields_error() {
        assert!(FilterFunction::parse_str("grayscale(foo)").is_err());
    }

    #[test]
    fn invalid_huerotate_yields_error() {
        assert!(FilterFunction::parse_str("hue-rotate(foo)").is_err());
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
