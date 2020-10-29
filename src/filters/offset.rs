use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::Node;
use crate::parsers::ParseValue;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// The `feOffset` filter primitive.
pub struct FeOffset {
    base: PrimitiveWithInput,
    dx: f64,
    dy: f64,
}

impl Default for FeOffset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    fn default() -> FeOffset {
        FeOffset {
            base: PrimitiveWithInput::new::<Self>(),
            dx: 0f64,
            dy: 0f64,
        }
    }
}

impl SetAttributes for FeOffset {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "dx") => self.dx = attr.parse(value)?,
                expanded_name!("", "dy") => self.dy = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeOffset {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, acquired_nodes, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx, node.parent().as_ref())?
            .add_input(&input)
            .into_irect(draw_ctx);

        let (dx, dy) = ctx.paffine().transform_distance(self.dx, self.dy);

        let surface = input.surface().offset(bounds, dx, dy)?;

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
