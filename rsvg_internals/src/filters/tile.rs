use cairo::{self, ImageSurface, Matrix, MatrixTrait, Pattern};

use drawing_ctx::DrawingCtx;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use property_bag::PropertyBag;
use surface_utils::shared_surface::SharedImageSurface;

use super::context::{FilterContext, FilterInput, FilterOutput, FilterResult};
use super::{Filter, FilterError, PrimitiveWithInput};

/// The `feTile` filter primitive.
pub struct Tile {
    base: PrimitiveWithInput,
}

impl Tile {
    /// Constructs a new `Tile` with empty properties.
    #[inline]
    pub fn new() -> Tile {
        Tile {
            base: PrimitiveWithInput::new::<Self>(),
        }
    }
}

impl NodeTrait for Tile {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)
    }
}

impl Filter for Tile {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
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
                let bounded_input_surface = ImageSurface::create(
                    cairo::Format::ARgb32,
                    input_bounds.x1 - input_bounds.x0,
                    input_bounds.y1 - input_bounds.y0,
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
                let mut mat = Matrix::identity();
                mat.translate(f64::from(-input_bounds.x0), f64::from(-input_bounds.y0));
                ptn.set_matrix(mat);

                let output_surface = ImageSurface::create(
                    cairo::Format::ARgb32,
                    ctx.source_graphic().width(),
                    ctx.source_graphic().height(),
                )?;

                {
                    let cr = cairo::Context::new(&output_surface);
                    cr.rectangle(
                        bounds.x0 as f64,
                        bounds.y0 as f64,
                        (bounds.x1 - bounds.x0) as f64,
                        (bounds.y1 - bounds.y0) as f64,
                    );
                    cr.clip();

                    cr.set_source(&ptn);
                    cr.paint();
                }

                SharedImageSurface::new(output_surface, surface.surface_type())?
            }
        };

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
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
