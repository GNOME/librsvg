use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::node::Node;
use crate::parsers::ParseValue;
use crate::property_defs::ColorInterpolationFilters;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, FilterRender, Input, Primitive};

/// The `feOffset` filter primitive.
pub struct FeOffset {
    base: Primitive,
    in1: Input,
    dx: f64,
    dy: f64,
}

impl Default for FeOffset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    fn default() -> FeOffset {
        FeOffset {
            base: Primitive::new(),
            in1: Default::default(),
            dx: 0f64,
            dy: 0f64,
        }
    }
}

impl SetAttributes for FeOffset {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.in1 = self.base.parse_one_input(attrs)?;

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

impl FilterRender for FeOffset {
    fn render(
        &self,
        _node: &Node,
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
        let bounds = self
            .base
            .get_bounds(ctx)?
            .add_input(&input_1)
            .into_irect(ctx, draw_ctx);

        let (dx, dy) = ctx.paffine().transform_distance(self.dx, self.dy);

        let surface = input_1.surface().offset(bounds, dx, dy)?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }
}

impl FilterEffect for FeOffset {}
