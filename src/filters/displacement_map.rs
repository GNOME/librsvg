use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::parsers::{Parse, ParseValue};
use crate::surface_utils::{iterators::Pixels, shared_surface::ExclusiveImageSurface};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, PrimitiveWithInput};

/// Enumeration of the color channels the displacement map can source.
#[derive(Clone, Copy)]
enum ColorChannel {
    R,
    G,
    B,
    A,
}

/// The `feDisplacementMap` filter primitive.
pub struct FeDisplacementMap {
    base: PrimitiveWithInput,
    in2: Option<Input>,
    scale: f64,
    x_channel_selector: ColorChannel,
    y_channel_selector: ColorChannel,
}

impl Default for FeDisplacementMap {
    /// Constructs a new `DisplacementMap` with empty properties.
    #[inline]
    fn default() -> FeDisplacementMap {
        FeDisplacementMap {
            base: PrimitiveWithInput::new::<Self>(),
            in2: None,
            scale: 0.0,
            x_channel_selector: ColorChannel::A,
            y_channel_selector: ColorChannel::A,
        }
    }
}

impl SetAttributes for FeDisplacementMap {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "in2") => self.in2 = Some(attr.parse(value)?),
                expanded_name!("", "scale") => self.scale = attr.parse(value)?,

                expanded_name!("", "xChannelSelector") => {
                    self.x_channel_selector = attr.parse(value)?
                }

                expanded_name!("", "yChannelSelector") => {
                    self.y_channel_selector = attr.parse(value)?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeDisplacementMap {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, acquired_nodes, draw_ctx)?;
        let displacement_input = ctx.get_input(acquired_nodes, draw_ctx, self.in2.as_ref())?;
        let bounds = self
            .base
            .get_bounds(ctx, node.parent().as_ref())?
            .add_input(&input)
            .add_input(&displacement_input)
            .into_irect(draw_ctx);

        // Displacement map's values need to be non-premultiplied.
        let displacement_surface = displacement_input.surface().unpremultiply(bounds)?;

        let (sx, sy) = ctx.paffine().transform_distance(self.scale, self.scale);

        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input.surface().surface_type(),
        )?;

        surface.draw(&mut |cr| {
            for (x, y, displacement_pixel) in Pixels::within(&displacement_surface, bounds) {
                let get_value = |channel| match channel {
                    ColorChannel::R => displacement_pixel.r,
                    ColorChannel::G => displacement_pixel.g,
                    ColorChannel::B => displacement_pixel.b,
                    ColorChannel::A => displacement_pixel.a,
                };

                let process = |x| f64::from(x) / 255.0 - 0.5;

                let dx = process(get_value(self.x_channel_selector));
                let dy = process(get_value(self.y_channel_selector));

                let x = f64::from(x);
                let y = f64::from(y);
                let ox = sx * dx;
                let oy = sy * dy;

                // Doing this in a loop doesn't look too bad performance wise, and allows not to
                // manually implement bilinear or other interpolation.
                cr.rectangle(x, y, 1.0, 1.0);
                cr.reset_clip();
                cr.clip();

                input.surface().set_as_source_surface(&cr, -ox, -oy);
                cr.paint();
            }

            Ok(())
        })?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: surface.share()?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        // Performance TODO: this converts in back and forth to linear RGB while technically it's
        // only needed for in2.
        true
    }
}

impl Parse for ColorChannel {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "R" => ColorChannel::R,
            "G" => ColorChannel::G,
            "B" => ColorChannel::B,
            "A" => ColorChannel::A,
        )?)
    }
}
