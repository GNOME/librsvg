use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::{CascadedValues, Node};
use crate::paint_server::resolve_color;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Primitive, PrimitiveParams, ResolvedPrimitive};

/// The `feFlood` filter primitive.
#[derive(Default)]
pub struct FeFlood {
    base: Primitive,
}

/// Resolved `feFlood` primitive for rendering.
pub struct Flood {
    color: cssparser::RGBA,
}

impl SetAttributes for FeFlood {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.parse_no_inputs(attrs)
    }
}

impl Flood {
    pub fn render(
        &self,
        primitive: &ResolvedPrimitive,
        ctx: &FilterContext,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let bounds = primitive.get_bounds(ctx)?.into_irect(ctx, draw_ctx);

        let surface = ctx.source_graphic().flood(bounds, self.color)?;

        Ok(FilterResult {
            name: primitive.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }
}

impl FilterEffect for FeFlood {
    fn resolve(&self, node: &Node) -> Result<(Primitive, PrimitiveParams), FilterError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        Ok((
            self.base.clone(),
            PrimitiveParams::Flood(Flood {
                color: resolve_color(
                    &values.flood_color().0,
                    values.flood_opacity().0,
                    values.color().0,
                ),
            }),
        ))
    }
}
