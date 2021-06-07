use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::Operator;
use crate::xml::Attributes;

use super::bounds::BoundsBuilder;
use super::context::{FilterContext, FilterOutput};
use super::{
    FilterEffect, FilterError, FilterResolveError, Input, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};

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
#[derive(Default)]
pub struct FeBlend {
    base: Primitive,
    params: Blend,
}

/// Resolved `feBlend` primitive for rendering.
#[derive(Clone, Default)]
pub struct Blend {
    in1: Input,
    in2: Input,
    mode: Mode,
    color_interpolation_filters: ColorInterpolationFilters,
}

impl SetAttributes for FeBlend {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        let (in1, in2) = self.base.parse_two_inputs(attrs)?;
        self.params.in1 = in1;
        self.params.in2 = in2;

        for (attr, value) in attrs.iter() {
            if let expanded_name!("", "mode") = attr.expanded() {
                self.params.mode = attr.parse(value)?;
            }
        }

        Ok(())
    }
}

impl Blend {
    pub fn render(
        &self,
        bounds_builder: BoundsBuilder,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterOutput, FilterError> {
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
        let bounds: IRect = bounds_builder
            .add_input(&input_1)
            .add_input(&input_2)
            .compute(ctx)
            .clipped
            .into();

        let surface = input_1
            .surface()
            .compose(input_2.surface(), bounds, self.mode.into())?;

        Ok(FilterOutput { surface, bounds })
    }
}

impl FilterEffect for FeBlend {
    fn resolve(
        &self,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        node: &Node,
    ) -> Result<ResolvedPrimitive, FilterResolveError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        let mut params = self.params.clone();
        params.color_interpolation_filters = values.color_interpolation_filters();

        Ok(ResolvedPrimitive {
            primitive: self.base.clone(),
            params: PrimitiveParams::Blend(params),
        })
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

impl From<Mode> for Operator {
    #[inline]
    fn from(x: Mode) -> Self {
        use Mode::*;

        match x {
            Normal => Operator::Over,
            Multiply => Operator::Multiply,
            Screen => Operator::Screen,
            Darken => Operator::Darken,
            Lighten => Operator::Lighten,
            Overlay => Operator::Overlay,
            ColorDodge => Operator::ColorDodge,
            ColorBurn => Operator::ColorBurn,
            HardLight => Operator::HardLight,
            SoftLight => Operator::SoftLight,
            Difference => Operator::Difference,
            Exclusion => Operator::Exclusion,
            HslHue => Operator::HslHue,
            HslSaturation => Operator::HslSaturation,
            HslColor => Operator::HslColor,
            HslLuminosity => Operator::HslLuminosity,
        }
    }
}
