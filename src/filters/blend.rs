use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{Parse, ParseValue};
use crate::property_defs::ColorInterpolationFilters;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams};

/// Enumeration of the possible blending modes.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum Mode {
    Normal,
    Multiply,
    Screen,
    Darken,
    Lighten,
    Overlay,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    HslHue,
    HslSaturation,
    HslColor,
    HslLuminosity,
}

enum_default!(Mode, Mode::Normal);

/// The `feBlend` filter primitive.
pub struct FeBlend {
    base: Primitive,
    in1: Input,
    in2: Input,
    mode: Mode,
}

/// Resolved `feBlend` primitive for rendering.
pub struct Blend {
    base: Primitive,
    in1: Input,
    in2: Input,
    mode: Mode,
    color_interpolation_filters: ColorInterpolationFilters,
}

impl Default for FeBlend {
    /// Constructs a new `Blend` with empty properties.
    #[inline]
    fn default() -> FeBlend {
        FeBlend {
            base: Primitive::new(),
            in1: Default::default(),
            in2: Default::default(),
            mode: Mode::default(),
        }
    }
}

impl SetAttributes for FeBlend {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        let (in1, in2) = self.base.parse_two_inputs(attrs)?;
        self.in1 = in1;
        self.in2 = in2;

        for (attr, value) in attrs.iter() {
            if let expanded_name!("", "mode") = attr.expanded() {
                self.mode = attr.parse(value)?;
            }
        }

        Ok(())
    }
}

impl Blend {
    pub fn render(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            self.color_interpolation_filters,
        )?;
        let input_2 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in2,
            self.color_interpolation_filters,
        )?;
        let bounds = self
            .base
            .get_bounds(ctx)?
            .add_input(&input_1)
            .add_input(&input_2)
            .into_irect(ctx, draw_ctx);

        let surface = input_1.surface().compose(
            input_2.surface(),
            bounds,
            cairo::Operator::from(self.mode),
        )?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }
}

impl FilterEffect for FeBlend {
    fn resolve(&self, node: &Node) -> Result<PrimitiveParams, FilterError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        Ok(PrimitiveParams::Blend(Blend {
            base: self.base.clone(),
            in1: self.in1.clone(),
            in2: self.in2.clone(),
            mode: self.mode,
            color_interpolation_filters: values.color_interpolation_filters(),
        }))
    }
}

impl Parse for Mode {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "normal" => Mode::Normal,
            "multiply" => Mode::Multiply,
            "screen" => Mode::Screen,
            "darken" => Mode::Darken,
            "lighten" => Mode::Lighten,
            "overlay" => Mode::Overlay,
            "color-dodge" => Mode::ColorDodge,
            "color-burn" => Mode::ColorBurn,
            "hard-light" => Mode::HardLight,
            "soft-light" => Mode::SoftLight,
            "difference" => Mode::Difference,
            "exclusion" => Mode::Exclusion,
            "hue" => Mode::HslHue,
            "saturation" => Mode::HslSaturation,
            "color" => Mode::HslColor,
            "luminosity" => Mode::HslLuminosity,
        )?)
    }
}

impl From<Mode> for cairo::Operator {
    #[inline]
    fn from(x: Mode) -> Self {
        match x {
            Mode::Normal => cairo::Operator::Over,
            Mode::Multiply => cairo::Operator::Multiply,
            Mode::Screen => cairo::Operator::Screen,
            Mode::Darken => cairo::Operator::Darken,
            Mode::Lighten => cairo::Operator::Lighten,
            Mode::Overlay => cairo::Operator::Overlay,
            Mode::ColorDodge => cairo::Operator::ColorDodge,
            Mode::ColorBurn => cairo::Operator::ColorBurn,
            Mode::HardLight => cairo::Operator::HardLight,
            Mode::SoftLight => cairo::Operator::SoftLight,
            Mode::Difference => cairo::Operator::Difference,
            Mode::Exclusion => cairo::Operator::Exclusion,
            Mode::HslHue => cairo::Operator::HslHue,
            Mode::HslSaturation => cairo::Operator::HslSaturation,
            Mode::HslColor => cairo::Operator::HslColor,
            Mode::HslLuminosity => cairo::Operator::HslLuminosity,
        }
    }
}
