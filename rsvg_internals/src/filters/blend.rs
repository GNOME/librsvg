use cairo;
use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::surface_utils::shared_surface::SharedImageSurface;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::input::Input;
use super::{FilterEffect, FilterError, PrimitiveWithInput};

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

impl NodeTrait for FeBlend {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "in2") => {
                    self.in2 = Some(Input::parse(attr, value)?);
                }
                expanded_name!(svg "mode") => self.mode = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeBlend {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let input_2 = ctx.get_input(draw_ctx, self.in2.as_ref())?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .add_input(&input_2)
            .into_irect(draw_ctx);

        // If we're combining two alpha-only surfaces, the result is alpha-only. Otherwise the
        // result is whatever the non-alpha-only type we're working on (which can be either sRGB or
        // linear sRGB depending on color-interpolation-filters).
        let surface_type = if input.surface().is_alpha_only() {
            input_2.surface().surface_type()
        } else {
            if !input_2.surface().is_alpha_only() {
                // All surface types should match (this is enforced by get_input()).
                assert_eq!(
                    input_2.surface().surface_type(),
                    input.surface().surface_type()
                );
            }

            input.surface().surface_type()
        };

        let output_surface = input_2.surface().copy_surface(bounds)?;
        {
            let cr = cairo::Context::new(&output_surface);
            let r = cairo::Rectangle::from(bounds);
            cr.rectangle(r.x, r.y, r.width, r.height);
            cr.clip();

            input.surface().set_as_source_surface(&cr, 0f64, 0f64);
            cr.set_operator(self.mode.into());
            cr.paint();
        }

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, surface_type)?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}

impl Parse for Mode {
    fn parse(parser: &mut Parser<'_, '_>) -> Result<Self, ValueErrorKind> {
        parse_identifiers!(
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
        )
        .map_err(|_| ValueErrorKind::parse_error("parse error"))
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
