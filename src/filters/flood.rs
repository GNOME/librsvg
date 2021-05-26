use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::{CascadedValues, Node};
use crate::paint_server::resolve_color;
use crate::rect::IRect;
use crate::xml::Attributes;

use super::bounds::BoundsBuilder;
use super::context::{FilterContext, FilterOutput};
use super::{
    FilterEffect, FilterError, FilterResolveError, Primitive, PrimitiveParams, ResolvedPrimitive,
};

/// The `feFlood` filter primitive.
#[derive(Default)]
pub struct FeFlood {
    base: Primitive,
}

/// Resolved `feFlood` primitive for rendering.
pub struct Flood {
    pub color: cssparser::RGBA,
}

impl SetAttributes for FeFlood {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.parse_no_inputs(attrs)
    }
}

impl Flood {
    pub fn render(
        &self,
        bounds_builder: BoundsBuilder,
        ctx: &FilterContext,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        _draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterOutput, FilterError> {
        let bounds: IRect = bounds_builder.compute(ctx).clipped.into();

        let surface = ctx.source_graphic().flood(bounds, self.color)?;

        Ok(FilterOutput { surface, bounds })
    }
}

impl FilterEffect for FeFlood {
    fn resolve(
        &self,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        node: &Node,
    ) -> Result<ResolvedPrimitive, FilterResolveError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        Ok(ResolvedPrimitive {
            primitive: self.base.clone(),
            params: PrimitiveParams::Flood(Flood {
                color: resolve_color(
                    &values.flood_color().0,
                    values.flood_opacity().0,
                    values.color().0,
                ),
            }),
        })
    }
}
