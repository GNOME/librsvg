use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::drawing_ctx::DrawingCtx;
use crate::node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::input::Input;
use super::{FilterEffect, FilterError, Primitive};

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

impl NodeTrait for FeMerge {
    impl_node_as_filter_effect!();

    #[inline]
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)
    }
}

impl NodeTrait for FeMergeNode {
    #[inline]
    fn set_atts(&mut self, _parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "in") => self.in_ = Some(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }
}

impl FeMergeNode {
    fn render(
        &self,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
        bounds: IRect,
        output_surface: Option<SharedImageSurface>,
    ) -> Result<SharedImageSurface, FilterError> {
        let input = ctx.get_input(draw_ctx, self.in_.as_ref())?;

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
        node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        // Compute the filter bounds, taking each child node's input into account.
        let mut bounds = self.base.get_bounds(ctx);
        for child in node
            .children()
            .filter(|c| c.borrow().get_type() == NodeType::FeMergeNode)
        {
            if child.borrow().is_in_error() {
                return Err(FilterError::ChildNodeInError);
            }

            let input = ctx.get_input(
                draw_ctx,
                child.borrow().get_impl::<FeMergeNode>().in_.as_ref(),
            )?;
            bounds = bounds.add_input(&input);
        }
        let bounds = bounds.into_irect(draw_ctx);

        // Now merge them all.
        let mut output_surface = None;
        for child in node
            .children()
            .filter(|c| c.borrow().get_type() == NodeType::FeMergeNode)
        {
            output_surface = Some(child.borrow().get_impl::<FeMergeNode>().render(
                ctx,
                draw_ctx,
                bounds,
                output_surface,
            )?);
        }

        let surface = match output_surface {
            Some(surface) => surface,
            None => SharedImageSurface::new(
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
