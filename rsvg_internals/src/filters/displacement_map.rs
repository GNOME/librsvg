use cairo::{self, ImageSurface, MatrixTrait};
use markup5ever::{local_name, LocalName};

use crate::drawing_ctx::DrawingCtx;
use crate::error::{AttributeResultExt, NodeError};
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{self, ParseError};
use crate::property_bag::PropertyBag;
use crate::surface_utils::{iterators::Pixels, shared_surface::SharedImageSurface};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{Filter, FilterError, Input, PrimitiveWithInput};

/// Enumeration of the color channels the displacement map can source.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum ColorChannel {
    R,
    G,
    B,
    A,
}

/// The `feDisplacementMap` filter primitive.
pub struct DisplacementMap {
    base: PrimitiveWithInput,
    in2: Option<Input>,
    scale: f64,
    x_channel_selector: ColorChannel,
    y_channel_selector: ColorChannel,
}

impl Default for DisplacementMap {
    /// Constructs a new `DisplacementMap` with empty properties.
    #[inline]
    fn default() -> DisplacementMap {
        DisplacementMap {
            base: PrimitiveWithInput::new::<Self>(),
            in2: None,
            scale: 0.0,
            x_channel_selector: ColorChannel::A,
            y_channel_selector: ColorChannel::A,
        }
    }
}

impl NodeTrait for DisplacementMap {
    impl_node_as_filter!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("in2") => self.in2 = Some(Input::parse(attr, value)?),
                local_name!("scale") => self.scale = parsers::number(value).attribute(attr)?,
                local_name!("xChannelSelector") => {
                    self.x_channel_selector = ColorChannel::parse(attr, value)?
                }
                local_name!("yChannelSelector") => {
                    self.y_channel_selector = ColorChannel::parse(attr, value)?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Filter for DisplacementMap {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let displacement_input = ctx.get_input(draw_ctx, self.in2.as_ref())?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .add_input(&displacement_input)
            .into_irect(draw_ctx);

        // Displacement map's values need to be non-premultiplied.
        let displacement_surface = displacement_input.surface().unpremultiply(bounds)?;

        let (sx, sy) = ctx.paffine().transform_distance(self.scale, self.scale);

        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        {
            let cr = cairo::Context::new(&output_surface);

            for (x, y, displacement_pixel) in Pixels::new(&displacement_surface, bounds) {
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
        }

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, input.surface().surface_type())?,
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

impl ColorChannel {
    fn parse(attr: LocalName, s: &str) -> Result<Self, NodeError> {
        match s {
            "R" => Ok(ColorChannel::R),
            "G" => Ok(ColorChannel::G),
            "B" => Ok(ColorChannel::B),
            "A" => Ok(ColorChannel::A),
            _ => Err(NodeError::parse_error(
                attr,
                ParseError::new("invalid value"),
            )),
        }
    }
}
