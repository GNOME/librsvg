use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{Parse, ParseValue};
use crate::property_defs::ColorInterpolationFilters;
use crate::surface_utils::{iterators::Pixels, shared_surface::ExclusiveImageSurface};
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, FilterRender, Input, Primitive};

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
    base: Primitive,
    in1: Input,
    in2: Input,
    scale: f64,
    x_channel_selector: ColorChannel,
    y_channel_selector: ColorChannel,
}

impl Default for FeDisplacementMap {
    /// Constructs a new `DisplacementMap` with empty properties.
    #[inline]
    fn default() -> FeDisplacementMap {
        FeDisplacementMap {
            base: Primitive::new(),
            in1: Default::default(),
            in2: Default::default(),
            scale: 0.0,
            x_channel_selector: ColorChannel::A,
            y_channel_selector: ColorChannel::A,
        }
    }
}

impl SetAttributes for FeDisplacementMap {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        let (in1, in2) = self.base.parse_two_inputs(attrs)?;
        self.in1 = in1;
        self.in2 = in2;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
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

impl FilterRender for FeDisplacementMap {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();
        let cif = values.color_interpolation_filters();

        // https://www.w3.org/TR/filter-effects/#feDisplacementMapElement
        // "The color-interpolation-filters property only applies to
        // the in2 source image and does not apply to the in source
        // image. The in source image must remain in its current color
        // space.

        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            ColorInterpolationFilters::Auto,
        )?;
        let displacement_input = ctx.get_input(acquired_nodes, draw_ctx, &self.in2, cif)?;
        let bounds = self
            .base
            .get_bounds(ctx)?
            .add_input(&input_1)
            .add_input(&displacement_input)
            .into_irect(ctx, draw_ctx);

        // Displacement map's values need to be non-premultiplied.
        let displacement_surface = displacement_input.surface().unpremultiply(bounds)?;

        let (sx, sy) = ctx.paffine().transform_distance(self.scale, self.scale);

        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input_1.surface().surface_type(),
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

                input_1.surface().set_as_source_surface(&cr, -ox, -oy);
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
}

impl FilterEffect for FeDisplacementMap {}

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
