use std::cell::Cell;

use cairo::{self, ImageSurface, MatrixTrait};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use surface_utils::shared_surface::SharedImageSurface;
use util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::{Filter, FilterError, PrimitiveWithInput};

/// The `feOffset` filter primitive.
pub struct Offset {
    base: PrimitiveWithInput,
    dx: Cell<f64>,
    dy: Cell<f64>,
}

impl Offset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    pub fn new() -> Offset {
        Offset {
            base: PrimitiveWithInput::new::<Self>(),
            dx: Cell::new(0f64),
            dy: Cell::new(0f64),
        }
    }
}

impl NodeTrait for Offset {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Dx => self.dx.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                Attribute::Dy => self.dy.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
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
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);

        let dx = self.dx.get();
        let dy = self.dy.get();
        let (ox, oy) = ctx.paffine().transform_distance(dx, dy);

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
            cr.rectangle(
                output_bounds.x0 as f64,
                output_bounds.y0 as f64,
                (output_bounds.x1 - output_bounds.x0) as f64,
                (output_bounds.y1 - output_bounds.y0) as f64,
            );
            cr.clip();

            input.surface().set_as_source_surface(&cr, ox, oy);
            cr.paint();
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
        false
    }
}
