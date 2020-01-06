use crate::drawing_ctx::DrawingCtx;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::property_bag::PropertyBag;
use crate::surface_utils::shared_surface::SharedImageSurface;

use super::context::{FilterContext, FilterInput, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// The `feTile` filter primitive.
pub struct FeTile {
    base: PrimitiveWithInput,
}

impl Default for FeTile {
    /// Constructs a new `Tile` with empty properties.
    #[inline]
    fn default() -> FeTile {
        FeTile {
            base: PrimitiveWithInput::new::<Self>(),
        }
    }
}

impl NodeTrait for FeTile {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)
    }
}

impl FilterEffect for FeTile {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;

        // feTile doesn't consider its inputs in the filter primitive subregion calculation.
        let bounds = self.base.get_bounds(ctx).into_irect(draw_ctx);

        let output_surface = match input {
            FilterInput::StandardInput(surface) => surface,
            FilterInput::PrimitiveOutput(FilterOutput {
                surface,
                bounds: input_bounds,
            }) => {
                // Create a surface containing just the region to tile.
                let bounded_input_surface = cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    input_bounds.width(),
                    input_bounds.height(),
                )?;

                {
                    let cr = cairo::Context::new(&bounded_input_surface);
                    surface.set_as_source_surface(
                        &cr,
                        f64::from(-input_bounds.x0),
                        f64::from(-input_bounds.y0),
                    );
                    cr.paint();
                }

                // Make a pattern out of the tile region.
                let ptn = cairo::SurfacePattern::create(&bounded_input_surface);
                ptn.set_extend(cairo::Extend::Repeat);
                let mut mat = cairo::Matrix::identity();
                mat.translate(f64::from(-input_bounds.x0), f64::from(-input_bounds.y0));
                ptn.set_matrix(mat);

                let output_surface = cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    ctx.source_graphic().width(),
                    ctx.source_graphic().height(),
                )?;

                {
                    let cr = cairo::Context::new(&output_surface);
                    let r = cairo::Rectangle::from(bounds);
                    cr.rectangle(r.x, r.y, r.width, r.height);
                    cr.clip();

                    cr.set_source(&ptn);
                    cr.paint();
                }

                SharedImageSurface::new(output_surface, surface.surface_type())?
            }
        };

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: output_surface,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}
