use cairo::{self, ImageSurface};
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::drawing_ctx::DrawingCtx;
use crate::error::AttributeResultExt;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers;
use crate::property_bag::PropertyBag;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{Filter, FilterError, PrimitiveWithInput};

/// The `feOffset` filter primitive.
pub struct Offset {
    base: PrimitiveWithInput,
    dx: f64,
    dy: f64,
}

impl Default for Offset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    fn default() -> Offset {
        Offset {
            base: PrimitiveWithInput::new::<Self>(),
            dx: 0f64,
            dy: 0f64,
        }
    }
}

impl NodeTrait for Offset {
    impl_node_as_filter!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "dx") => self.dx = parsers::number(value).attribute(attr)?,
                expanded_name!(svg "dy") => self.dy = parsers::number(value).attribute(attr)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Filter for Offset {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);

        let (ox, oy) = ctx.paffine().transform_distance(self.dx, self.dy);

        // output_bounds contains all pixels within bounds,
        // for which (x - ox) and (y - oy) also lie within bounds.
        let output_bounds = IRect {
            x0: clamp(bounds.x0 + ox as i32, bounds.x0, bounds.x1),
            y0: clamp(bounds.y0 + oy as i32, bounds.y0, bounds.y1),
            x1: clamp(bounds.x1 + ox as i32, bounds.x0, bounds.x1),
            y1: clamp(bounds.y1 + oy as i32, bounds.y0, bounds.y1),
        };

        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        {
            let cr = cairo::Context::new(&output_surface);
            let r = cairo::Rectangle::from(output_bounds);
            cr.rectangle(r.x, r.y, r.width, r.height);
            cr.clip();

            input.surface().set_as_source_surface(&cr, ox, oy);
            cr.paint();
        }

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, input.surface().surface_type())?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}
