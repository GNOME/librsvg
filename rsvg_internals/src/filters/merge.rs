use std::cell::RefCell;

use cairo::{self, ImageSurface};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use property_bag::PropertyBag;
use surface_utils::shared_surface::SharedImageSurface;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::input::Input;
use super::{Filter, FilterError, Primitive};

/// The `feMerge` filter primitive.
pub struct Merge {
    base: Primitive,
}

/// The `<feMergeNode>` element.
pub struct MergeNode {
    in_: RefCell<Option<Input>>,
}

impl Merge {
    /// Constructs a new `Merge` with empty properties.
    #[inline]
    pub fn new() -> Merge {
        Merge {
            base: Primitive::new::<Self>(),
        }
    }
}

impl MergeNode {
    /// Constructs a new `MergeNode` with empty properties.
    #[inline]
    pub fn new() -> MergeNode {
        MergeNode {
            in_: RefCell::new(None),
        }
    }
}

impl NodeTrait for Merge {
    #[inline]
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)
    }
}

impl NodeTrait for MergeNode {
    #[inline]
    fn set_atts(
        &self,
        _node: &RsvgNode,
        _handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::In => {
                    self.in_.replace(Some(Input::parse(Attribute::In, value)?));
                }
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
        output_surface: Option<ImageSurface>,
    ) -> Result<ImageSurface, FilterError> {
        let input = ctx.get_input(draw_ctx, self.in_.borrow().as_ref())?;

        if output_surface.is_none() {
            return input
                .surface()
                .copy_surface(bounds)
                .map_err(FilterError::IntermediateSurfaceCreation);
        }
        let output_surface = output_surface.unwrap();

        let cr = cairo::Context::new(&output_surface);
        cr.rectangle(
            bounds.x0 as f64,
            bounds.y0 as f64,
            (bounds.x1 - bounds.x0) as f64,
            (bounds.y1 - bounds.y0) as f64,
        );
        cr.clip();

        input.surface().set_as_source_surface(&cr, 0f64, 0f64);
        cr.set_operator(cairo::Operator::Over);
        cr.paint();

        Ok(output_surface)
    }
}

impl Filter for Merge {
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
            .filter(|c| c.get_type() == NodeType::FilterPrimitiveMergeNode)
        {
            bounds = bounds.add_input(
                &child.with_impl(|c: &MergeNode| ctx.get_input(draw_ctx, c.in_.borrow().as_ref()))?,
            );
        }
        let bounds = bounds.into_irect(draw_ctx);

        // Now merge them all.
        let mut output_surface = None;
        for child in node
            .children()
            .filter(|c| c.get_type() == NodeType::FilterPrimitiveMergeNode)
        {
            output_surface = Some(
                child.with_impl(|c: &MergeNode| c.render(ctx, draw_ctx, bounds, output_surface))?,
            );
        }

        let output_surface = match output_surface {
            Some(surface) => surface,
            None => ImageSurface::create(
                cairo::Format::ARgb32,
                ctx.source_graphic().width(),
                ctx.source_graphic().height(),
            ).map_err(FilterError::IntermediateSurfaceCreation)?,
        };

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface)
                    .map_err(FilterError::BadIntermediateSurfaceStatus)?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}
