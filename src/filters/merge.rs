use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::node::{Node, NodeBorrow};
use crate::parsers::ParseValue;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive};

/// The `feMerge` filter primitive.
pub struct FeMerge {
    base: Primitive,
}

/// The `<feMergeNode>` element.
#[derive(Default)]
pub struct FeMergeNode {
    in_: Option<Input>,
}

impl Default for FeMerge {
    /// Constructs a new `Merge` with empty properties.
    #[inline]
    fn default() -> FeMerge {
        FeMerge {
            base: Primitive::new::<Self>(),
        }
    }
}

impl SetAttributes for FeMerge {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)
    }
}

impl SetAttributes for FeMergeNode {
    #[inline]
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.in_ = attrs
            .iter()
            .find(|(attr, _)| attr.expanded() == expanded_name!("", "in"))
            .and_then(|(attr, value)| attr.parse(value).ok());

        Ok(())
    }
}

impl Draw for FeMergeNode {}

impl FeMergeNode {
    fn render(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
        bounds: IRect,
        output_surface: Option<SharedImageSurface>,
    ) -> Result<SharedImageSurface, FilterError> {
        let input = ctx.get_input(acquired_nodes, draw_ctx, self.in_.as_ref())?;

        if output_surface.is_none() {
            return Ok(input.surface().clone());
        }

        input
            .surface()
            .compose(&output_surface.unwrap(), bounds, cairo::Operator::Over)
            .map_err(FilterError::CairoError)
    }
}

impl FilterEffect for FeMerge {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        // Compute the filter bounds, taking each child node's input into account.
        let mut bounds = self.base.get_bounds(ctx, node.parent().as_ref())?;
        for child in node.children().filter(|c| c.is_element()) {
            let elt = child.borrow_element();

            if elt.is_in_error() {
                return Err(FilterError::ChildNodeInError);
            }

            if let Element::FeMergeNode(ref merge_node) = *elt {
                let input = ctx.get_input(acquired_nodes, draw_ctx, merge_node.in_.as_ref())?;
                bounds = bounds.add_input(&input);
            }
        }

        let bounds = bounds.into_irect(draw_ctx);

        // Now merge them all.
        let mut output_surface = None;
        for child in node.children().filter(|c| c.is_element()) {
            if let Element::FeMergeNode(ref merge_node) = *child.borrow_element() {
                output_surface = Some(merge_node.render(
                    ctx,
                    acquired_nodes,
                    draw_ctx,
                    bounds,
                    output_surface,
                )?);
            }
        }

        let surface = match output_surface {
            Some(s) => s,
            None => SharedImageSurface::empty(
                ctx.source_graphic().width(),
                ctx.source_graphic().height(),
                SurfaceType::AlphaOnly,
            )?,
        };

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}
