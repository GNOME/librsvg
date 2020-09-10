use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::Node;

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

impl SetAttributes for FeTile {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)
    }
}

impl FilterEffect for FeTile {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, acquired_nodes, draw_ctx)?;

        // feTile doesn't consider its inputs in the filter primitive subregion calculation.
        let bounds = self
            .base
            .get_bounds(ctx, node.parent().as_ref())?
            .into_irect(draw_ctx);

        let surface = match input {
            FilterInput::StandardInput(input_surface) => input_surface,
            FilterInput::PrimitiveOutput(FilterOutput {
                surface: input_surface,
                bounds: input_bounds,
            }) => {
                let tile_surface = input_surface.tile(input_bounds)?;

                ctx.source_graphic().paint_image_tiled(
                    bounds,
                    &tile_surface,
                    input_bounds.x0,
                    input_bounds.y0,
                )?
            }
        };

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}
