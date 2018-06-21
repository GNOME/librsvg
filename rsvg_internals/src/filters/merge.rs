use std::cell::RefCell;

use cairo::{self, ImageSurface};

use attributes::Attribute;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use property_bag::PropertyBag;
use srgb::{linearize_surface, unlinearize_surface};
use state::ColorInterpolationFilters;
use state::ComputedValues;
use surface_utils::shared_surface::SharedImageSurface;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::input::Input;
use super::{make_result, Filter, FilterError, Primitive};

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

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
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
        values: &ComputedValues,
        bounds: IRect,
        output_surface: Option<ImageSurface>,
    ) -> Result<ImageSurface, FilterError> {
        let input = make_result(ctx.get_input(self.in_.borrow().as_ref()))?;

        let input_surface =
            if values.color_interpolation_filters == ColorInterpolationFilters::LinearRgb {
                SharedImageSurface::new(
                    linearize_surface(input.surface(), bounds)
                        .map_err(FilterError::BadInputSurfaceStatus)?,
                ).unwrap()
            } else {
                input.surface().clone()
            };

        if output_surface.is_none() {
            return Ok(input_surface.into_image_surface());
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

        input_surface.set_as_source_surface(&cr, 0f64, 0f64);
        cr.set_operator(cairo::Operator::Over);
        cr.paint();

        Ok(output_surface)
    }
}

impl Filter for Merge {
    fn render(&self, node: &RsvgNode, ctx: &FilterContext) -> Result<FilterResult, FilterError> {
        // Compute the filter bounds, taking each child node's input into account.
        let mut bounds = self.base.get_bounds(ctx);
        for child in node
            .children()
            .filter(|c| c.get_type() == NodeType::FilterPrimitiveMergeNode)
        {
            bounds = bounds.add_input(&child.with_impl(move |c: &MergeNode| {
                make_result(ctx.get_input(c.in_.borrow().as_ref()))
            })?);
        }
        let bounds = bounds.into_irect();

        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        // Now merge them all.
        let mut output_surface = None;
        for child in node
            .children()
            .filter(|c| c.get_type() == NodeType::FilterPrimitiveMergeNode)
        {
            output_surface = Some(
                child
                    .with_impl(move |c: &MergeNode| c.render(ctx, values, bounds, output_surface))?,
            );
        }

        let output_surface = output_surface
            .map(|surface| {
                if values.color_interpolation_filters == ColorInterpolationFilters::LinearRgb {
                    unlinearize_surface(&SharedImageSurface::new(surface).unwrap(), bounds)
                } else {
                    Ok(surface)
                }
            })
            .unwrap_or_else(|| {
                ImageSurface::create(
                    cairo::Format::ARgb32,
                    ctx.source_graphic().width(),
                    ctx.source_graphic().height(),
                )
            })
            .map_err(FilterError::OutputSurfaceCreation)?;

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface).unwrap(),
                bounds,
            },
        })
    }
}
