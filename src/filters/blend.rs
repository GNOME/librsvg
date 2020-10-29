use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::parsers::{Parse, ParseValue};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, PrimitiveWithInput};

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

/// The `feBlend` filter primitive.
pub struct FeBlend {
    base: PrimitiveWithInput,
    in2: Option<Input>,
    mode: Mode,
}

impl Default for FeBlend {
    /// Constructs a new `Blend` with empty properties.
    #[inline]
    fn default() -> FeBlend {
        FeBlend {
            base: PrimitiveWithInput::new::<Self>(),
            in2: None,
            mode: Mode::Normal,
        }
    }
}

impl SetAttributes for FeBlend {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "in2") => {
                    self.in2 = Some(attr.parse(value)?);
                }
                expanded_name!("", "mode") => self.mode = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeBlend {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, acquired_nodes, draw_ctx)?;
        let input_2 = ctx.get_input(acquired_nodes, draw_ctx, self.in2.as_ref())?;
        let bounds = self
            .base
            .get_bounds(ctx, node.parent().as_ref())?
            .add_input(&input)
            .add_input(&input_2)
            .into_irect(draw_ctx);

        let surface =
            input
                .surface()
                .compose(input_2.surface(), bounds, cairo::Operator::from(self.mode))?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
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
