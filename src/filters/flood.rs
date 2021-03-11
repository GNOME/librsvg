use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::{CascadedValues, Node};
use crate::paint_server::resolve_color;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Primitive, PrimitiveParams};

/// The `feFlood` filter primitive.
pub struct FeFlood {
    base: Primitive,
}

/// Resolved `feFlood` primitive for rendering.
pub struct Flood {
    base: Primitive,
    color: cssparser::RGBA,
}

impl Default for FeFlood {
    /// Constructs a new `Flood` with empty properties.
    #[inline]
    fn default() -> FeFlood {
        FeFlood {
            base: Primitive::new(),
        }
    }
}

impl SetAttributes for FeFlood {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.parse_no_inputs(attrs)
    }
}

impl Flood {
    pub fn render(
        &self,
        ctx: &FilterContext,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let bounds = self.base.get_bounds(ctx)?.into_irect(ctx, draw_ctx);

        let surface = ctx.source_graphic().flood(bounds, self.color)?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }
}

impl FilterEffect for FeFlood {
    fn resolve(&self, node: &Node) -> Result<PrimitiveParams, FilterError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        Ok(PrimitiveParams::Flood(Flood {
            base: self.base.clone(),
            color: resolve_color(
                &values.flood_color().0,
                values.flood_opacity().0,
                values.color().0,
            ),
        }))
    }
}
