use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::Node;
use crate::property_defs::ColorInterpolationFilters;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterInput, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams};

/// The `feTile` filter primitive.
#[derive(Clone)]
pub struct FeTile {
    base: Primitive,
    in1: Input,
}

/// Resolved `feTile` primitive for rendering.
pub type Tile = FeTile;

impl Default for FeTile {
    /// Constructs a new `Tile` with empty properties.
    #[inline]
    fn default() -> FeTile {
        FeTile {
            base: Primitive::new(),
            in1: Default::default(),
        }
    }
}

impl SetAttributes for FeTile {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.in1 = self.base.parse_one_input(attrs)?;
        Ok(())
    }
}

impl FeTile {
    pub fn render(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        // https://www.w3.org/TR/filter-effects/#ColorInterpolationFiltersProperty
        //
        // "Note: The color-interpolation-filters property just has an
        // effect on filter operations. Therefore, it has no effect on
        // filter primitives like [...], feTile"
        //
        // This is why we pass Auto here.
        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            ColorInterpolationFilters::Auto,
        )?;

        // feTile doesn't consider its inputs in the filter primitive subregion calculation.
        let bounds = self.base.get_bounds(ctx)?.into_irect(ctx, draw_ctx);

        let surface = match input_1 {
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
}

impl FilterEffect for FeTile {
    fn resolve(&self, _node: &Node) -> Result<PrimitiveParams, FilterError> {
        Ok(PrimitiveParams::Tile(self.clone()))
    }
}
