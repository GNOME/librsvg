use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::Node;
use crate::parsers::ParseValue;
use crate::property_defs::ColorInterpolationFilters;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams, ResolvedPrimitive};

/// The `feOffset` filter primitive.
#[derive(Default)]
pub struct FeOffset {
    base: Primitive,
    params: Offset,
}

/// Resolved `feOffset` primitive for rendering.
#[derive(Clone, Default)]
pub struct Offset {
    in1: Input,
    dx: f64,
    dy: f64,
}

impl SetAttributes for FeOffset {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.params.in1 = self.base.parse_one_input(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "dx") => self.params.dx = attr.parse(value)?,
                expanded_name!("", "dy") => self.params.dy = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Offset {
    pub fn render(
        &self,
        primitive: &ResolvedPrimitive,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        // https://www.w3.org/TR/filter-effects/#ColorInterpolationFiltersProperty
        //
        // "Note: The color-interpolation-filters property just has an
        // effect on filter operations. Therefore, it has no effect on
        // filter primitives like feOffset"
        //
        // This is why we pass Auto here.
        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            ColorInterpolationFilters::Auto,
        )?;
        let bounds = primitive
            .get_bounds(ctx)
            .add_input(&input_1)
            .into_irect(ctx);

        let (dx, dy) = ctx.paffine().transform_distance(self.dx, self.dy);

        let surface = input_1.surface().offset(bounds, dx, dy)?;

        Ok(FilterResult {
            name: primitive.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }
}

impl FilterEffect for FeOffset {
    fn resolve(&self, _node: &Node) -> Result<(Primitive, PrimitiveParams), FilterError> {
        Ok((
            self.base.clone(),
            PrimitiveParams::Offset(self.params.clone()),
        ))
    }
}
