use cairo::{self, ImageSurface};
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::drawing_ctx::DrawingCtx;
use crate::node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::property_bag::PropertyBag;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::input::Input;
use super::{FilterEffect, FilterError, Primitive};

/// The `feMerge` filter primitive.
pub struct Merge {
    base: Primitive,
}

/// The `<feMergeNode>` element.
#[derive(Default)]
pub struct MergeNode {
    in_: Option<Input>,
}

impl Default for Merge {
    /// Constructs a new `Merge` with empty properties.
    #[inline]
    fn default() -> Merge {
        Merge {
            base: Primitive::new::<Self>(),
        }
    }
}

impl NodeTrait for Merge {
    impl_node_as_filter_effect!();

    #[inline]
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)
    }
}

impl NodeTrait for MergeNode {
    #[inline]
    fn set_atts(&mut self, _parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "in") => self.in_ = Some(Input::parse(attr, value)?),
                _ => (),
            }
        }

        Ok(())
    }
}

impl MergeNode {
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
        let output_surface = output_surface.unwrap();

        // If we're combining two alpha-only surfaces, the result is alpha-only. Otherwise the
        // result is whatever the non-alpha-only type we're working on (which can be either sRGB or
        // linear sRGB depending on color-interpolation-filters).
        let surface_type = if input.surface().is_alpha_only() {
            output_surface.surface_type()
        } else {
            if !output_surface.is_alpha_only() {
                // All surface types should match (this is enforced by get_input()).
                assert_eq!(
                    output_surface.surface_type(),
                    input.surface().surface_type()
                );
            }

            input.surface().surface_type()
        };

        let output_surface = output_surface.into_image_surface()?;

        {
            let cr = cairo::Context::new(&output_surface);
            let r = cairo::Rectangle::from(bounds);
            cr.rectangle(r.x, r.y, r.width, r.height);
            cr.clip();

            input.surface().set_as_source_surface(&cr, 0f64, 0f64);
            cr.set_operator(cairo::Operator::Over);
            cr.paint();
        }

        Ok(SharedImageSurface::new(output_surface, surface_type)?)
    }
}

impl FilterEffect for Merge {
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
                child.borrow().get_impl::<MergeNode>().in_.as_ref(),
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
            output_surface = Some(child.borrow().get_impl::<MergeNode>().render(
                ctx,
                draw_ctx,
                bounds,
                output_surface,
            )?);
        }

        let output_surface = match output_surface {
            Some(surface) => surface,
            None => SharedImageSurface::new(
                ImageSurface::create(
                    cairo::Format::ARgb32,
                    ctx.source_graphic().width(),
                    ctx.source_graphic().height(),
                )?,
                SurfaceType::AlphaOnly,
            )?,
        };

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: output_surface,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}
