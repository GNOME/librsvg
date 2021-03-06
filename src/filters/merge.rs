use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::parsers::ParseValue;
use crate::property_defs::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams};

/// The `feMerge` filter primitive.
pub struct FeMerge {
    base: Primitive,
}

/// The `<feMergeNode>` element.
#[derive(Clone, Default)]
pub struct FeMergeNode {
    in1: Input,
}

/// Resolved `feMerge` primitive for rendering.
pub struct Merge {
    base: Primitive,
    merge_nodes: Vec<MergeNode>,
}

/// Resolved `feMergeNode` for rendering.
#[derive(Debug, PartialEq)]
struct MergeNode {
    in1: Input,
    color_interpolation_filters: ColorInterpolationFilters,
}

impl Default for FeMerge {
    /// Constructs a new `Merge` with empty properties.
    #[inline]
    fn default() -> FeMerge {
        FeMerge {
            base: Primitive::new(),
        }
    }
}

impl SetAttributes for FeMerge {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.parse_no_inputs(attrs)
    }
}

impl SetAttributes for FeMergeNode {
    #[inline]
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            if let expanded_name!("", "in") = attr.expanded() {
                self.in1 = attr.parse(value)?;
            }
        }

        Ok(())
    }
}

impl Draw for FeMergeNode {}

impl MergeNode {
    fn render(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
        bounds: IRect,
        output_surface: Option<SharedImageSurface>,
    ) -> Result<SharedImageSurface, FilterError> {
        let input = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            self.color_interpolation_filters,
        )?;

        if output_surface.is_none() {
            return Ok(input.surface().clone());
        }

        input
            .surface()
            .compose(&output_surface.unwrap(), bounds, cairo::Operator::Over)
            .map_err(FilterError::CairoError)
    }
}

impl Merge {
    pub fn render(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        // Compute the filter bounds, taking each feMergeNode's input into account.
        let mut bounds = self.base.get_bounds(ctx)?;
        for merge_node in &self.merge_nodes {
            let input = ctx.get_input(
                acquired_nodes,
                draw_ctx,
                &merge_node.in1,
                merge_node.color_interpolation_filters,
            )?;
            bounds = bounds.add_input(&input);
        }

        let bounds = bounds.into_irect(ctx, draw_ctx);

        // Now merge them all.
        let mut output_surface = None;
        for merge_node in &self.merge_nodes {
            output_surface = merge_node
                .render(ctx, acquired_nodes, draw_ctx, bounds, output_surface)
                .ok();
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
}

impl FilterEffect for FeMerge {
    fn resolve(&self, node: &Node) -> Result<PrimitiveParams, FilterError> {
        Ok(PrimitiveParams::Merge(Merge {
            base: self.base.clone(),
            merge_nodes: resolve_merge_nodes(node)?,
        }))
    }
}

/// Takes a feMerge and walks its children to produce a list of feMergeNode arguments.
fn resolve_merge_nodes(node: &Node) -> Result<Vec<MergeNode>, FilterError> {
    let mut merge_nodes = Vec::new();

    for child in node.children().filter(|c| c.is_element()) {
        let elt = child.borrow_element();

        if elt.is_in_error() {
            return Err(FilterError::ChildNodeInError);
        }

        let cascaded = CascadedValues::new_from_node(&child);
        let values = cascaded.get();

        if let Element::FeMergeNode(ref merge_node) = *elt {
            merge_nodes.push(MergeNode {
                in1: merge_node.element_impl.in1.clone(),
                color_interpolation_filters: values.color_interpolation_filters(),
            });
        }
    }

    Ok(merge_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn extracts_parameters() {
        let document = Document::load_from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <filter id="filter">
    <feMerge id="merge">
      <feMergeNode in="SourceGraphic"/>
      <feMergeNode in="SourceAlpha" color-interpolation-filters="sRGB"/>
    </feMerge>
  </filter>
</svg>
"#,
        );

        let node = document.lookup_internal_node("merge").unwrap();
        let merge = borrow_element_as!(node, FeMerge);
        let params = merge.resolve(&node).unwrap();
        let params = match params {
            PrimitiveParams::Merge(m) => m,
            _ => unreachable!(),
        };
        assert_eq!(
            &params.merge_nodes[..],
            vec![
                MergeNode {
                    in1: Input::SourceGraphic,
                    color_interpolation_filters: Default::default(),
                },
                MergeNode {
                    in1: Input::SourceAlpha,
                    color_interpolation_filters: ColorInterpolationFilters::Srgb,
                },
            ]
        );
    }
}
