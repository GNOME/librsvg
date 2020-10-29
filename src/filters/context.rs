use std::cell::RefCell;
use std::collections::HashMap;
use std::f64;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::{DrawingCtx, ViewParams};
use crate::node::{Node, NodeBorrow};
use crate::parsers::CustomIdent;
use crate::properties::ComputedValues;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
use crate::transform::Transform;

use super::error::FilterError;
use super::Input;

/// A filter primitive output.
#[derive(Debug, Clone)]
pub struct FilterOutput {
    /// The surface after the filter primitive was applied.
    pub surface: SharedImageSurface,

    /// The filter primitive subregion.
    pub bounds: IRect,
}

/// A filter primitive result.
#[derive(Debug, Clone)]
pub struct FilterResult {
    /// The name of this result: the value of the `result` attribute.
    pub name: Option<CustomIdent>,

    /// The output.
    pub output: FilterOutput,
}

/// An input to a filter primitive.
#[derive(Debug, Clone)]
pub enum FilterInput {
    /// One of the standard inputs.
    StandardInput(SharedImageSurface),
    /// Output of another filter primitive.
    PrimitiveOutput(FilterOutput),
}

/// The filter rendering context.
pub struct FilterContext {
    /// The <filter> node.
    node: Node,
    /// Bounding box of node being filtered
    node_bbox: BoundingBox,
    /// Values from the node which referenced this filter.
    computed_from_node_being_filtered: ComputedValues,
    /// The source graphic surface.
    source_surface: SharedImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<FilterOutput>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<CustomIdent, FilterOutput>,
    /// The background surface. Computed lazily.
    background_surface: RefCell<Option<Result<SharedImageSurface, FilterError>>>,
    /// The filter effects region.
    effects_region: BoundingBox,
    /// Whether the currently rendered filter primitive uses linear RGB for color operations.
    ///
    /// This affects `get_input()` and `store_result()` which should perform linearization and
    /// unlinearization respectively when this is set to `true`.
    processing_linear_rgb: bool,

    /// The filter element affine matrix.
    ///
    /// If `filterUnits == userSpaceOnUse`, equal to the drawing context matrix, so, for example,
    /// if the target node is in a group with `transform="translate(30, 20)"`, this will be equal
    /// to a matrix that translates to 30, 20 (and does not scale). Note that the target node
    /// bounding box isn't included in the computations in this case.
    ///
    /// If `filterUnits == objectBoundingBox`, equal to the target node bounding box matrix
    /// multiplied by the drawing context matrix, so, for example, if the target node is in a group
    /// with `transform="translate(30, 20)"` and also has `x="1", y="1", width="50", height="50"`,
    /// this will be equal to a matrix that translates to 31, 21 and scales to 50, 50.
    ///
    /// This is to be used in conjunction with setting the viewbox size to account for the scaling.
    /// For `filterUnits == userSpaceOnUse`, the viewbox will have the actual resolution size, and
    /// for `filterUnits == objectBoundingBox`, the viewbox will have the size of 1, 1.
    _affine: Transform,

    /// The filter primitive affine matrix.
    ///
    /// See the comments for `_affine`, they largely apply here.
    paffine: Transform,
}

impl FilterContext {
    /// Creates a new `FilterContext`.
    pub fn new(
        filter_node: &Node,
        computed_from_node_being_filtered: &ComputedValues,
        source_surface: SharedImageSurface,
        draw_ctx: &mut DrawingCtx,
        draw_transform: Transform,
        node_bbox: BoundingBox,
    ) -> Self {
        // The rect can be empty (for example, if the filter is applied to an empty group).
        // However, with userSpaceOnUse it's still possible to create images with a filter.
        let bbox_rect = node_bbox.rect.unwrap_or_default();

        let filter = borrow_element_as!(filter_node, Filter);

        let affine = match filter.get_filter_units() {
            CoordUnits::UserSpaceOnUse => draw_transform,
            CoordUnits::ObjectBoundingBox => Transform::new_unchecked(
                bbox_rect.width(),
                0.0,
                0.0,
                bbox_rect.height(),
                bbox_rect.x0,
                bbox_rect.y0,
            )
            .post_transform(&draw_transform),
        };

        let paffine = match filter.get_primitive_units() {
            CoordUnits::UserSpaceOnUse => draw_transform,
            CoordUnits::ObjectBoundingBox => Transform::new_unchecked(
                bbox_rect.width(),
                0.0,
                0.0,
                bbox_rect.height(),
                bbox_rect.x0,
                bbox_rect.y0,
            )
            .post_transform(&draw_transform),
        };

        let (width, height) = (source_surface.width(), source_surface.height());

        Self {
            node: filter_node.clone(),
            node_bbox,
            computed_from_node_being_filtered: computed_from_node_being_filtered.clone(),
            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
            background_surface: RefCell::new(None),
            effects_region: filter.compute_effects_region(
                computed_from_node_being_filtered,
                draw_ctx,
                affine,
                f64::from(width),
                f64::from(height),
            ),
            processing_linear_rgb: false,
            _affine: affine,
            paffine,
        }
    }

    /// Returns the computed values from the node that referenced this filter.
    #[inline]
    pub fn get_computed_values_from_node_being_filtered(&self) -> &ComputedValues {
        &self.computed_from_node_being_filtered
    }

    /// Returns the surface corresponding to the source graphic.
    #[inline]
    pub fn source_graphic(&self) -> &SharedImageSurface {
        &self.source_surface
    }

    /// Returns the surface corresponding to the background image snapshot.
    pub fn background_image(
        &self,
        draw_ctx: &DrawingCtx,
    ) -> Result<SharedImageSurface, FilterError> {
        {
            // At this point either no, or only immutable references to background_surface exist, so
            // it's ok to make an immutable reference.
            let bg = self.background_surface.borrow();

            // If background_surface was already computed, return the immutable reference. It will
            // get bound to the &self lifetime by the function return type.
            if let Some(ref result) = *bg {
                return result.clone();
            }
        }

        // If we got here, then background_surface hasn't been computed yet. This means there are
        // no references to it and we can create a mutable reference.
        let mut bg = self.background_surface.borrow_mut();

        *bg = Some(
            draw_ctx
                .get_snapshot(self.source_surface.width(), self.source_surface.height())
                .map_err(FilterError::CairoError),
        );

        // Return the only existing reference as immutable.
        bg.as_ref()
            .unwrap()
            .as_ref()
            .map(|surf| surf.clone())
            .map_err(|&s| s)
    }

    /// Converts this `FilterContext` into the surface corresponding to the output of the filter
    /// chain.
    ///
    /// The returned surface is in the sRGB color space.
    // TODO: sRGB conversion should probably be done by the caller.
    #[inline]
    pub fn into_output(self) -> Result<SharedImageSurface, cairo::Status> {
        match self.last_result {
            Some(FilterOutput { surface, bounds }) => surface.to_srgb(bounds),
            None => SharedImageSurface::empty(
                self.source_surface.width(),
                self.source_surface.height(),
                SurfaceType::AlphaOnly,
            ),
        }
    }

    /// Stores a filter primitive result into the context.
    #[inline]
    pub fn store_result(&mut self, result: FilterResult) -> Result<(), FilterError> {
        if let Some(name) = result.name {
            self.previous_results.insert(name, result.output.clone());
        }

        self.last_result = Some(result.output);
        Ok(())
    }

    /// Returns the paffine matrix.
    #[inline]
    pub fn paffine(&self) -> Transform {
        self.paffine
    }

    /// Returns the filter effects region.
    #[inline]
    pub fn effects_region(&self) -> BoundingBox {
        self.effects_region
    }

    pub fn get_computed_from_node_being_filtered(&self) -> &ComputedValues {
        &self.computed_from_node_being_filtered
    }

    /// Pushes the viewport size based on the value of `primitiveUnits`.
    pub fn get_view_params(&self, draw_ctx: &mut DrawingCtx) -> ViewParams {
        // See comments in compute_effects_region() for how this works.
        let units = borrow_element_as!(self.node, Filter).get_primitive_units();
        draw_ctx.push_coord_units(units)
    }

    /// Retrieves the filter input surface according to the SVG rules.
    ///
    /// Does not take `processing_linear_rgb` into account.
    fn get_input_raw(
        &self,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
        in_: Option<&Input>,
    ) -> Result<FilterInput, FilterError> {
        if in_.is_none() {
            // No value => use the last result.
            // As per the SVG spec, if the filter primitive is the first in the chain, return the
            // source graphic.
            if let Some(output) = self.last_result.as_ref() {
                return Ok(FilterInput::PrimitiveOutput(output.clone()));
            } else {
                return Ok(FilterInput::StandardInput(self.source_graphic().clone()));
            }
        }

        let values = &self.computed_from_node_being_filtered;

        match *in_.unwrap() {
            Input::SourceGraphic => Ok(FilterInput::StandardInput(self.source_graphic().clone())),

            Input::SourceAlpha => self
                .source_graphic()
                .extract_alpha(self.effects_region().rect.unwrap().into())
                .map_err(FilterError::CairoError)
                .map(FilterInput::StandardInput),

            Input::BackgroundImage => self
                .background_image(draw_ctx)
                .map(FilterInput::StandardInput),

            Input::BackgroundAlpha => self
                .background_image(draw_ctx)
                .and_then(|surface| {
                    surface
                        .extract_alpha(self.effects_region().rect.unwrap().into())
                        .map_err(FilterError::CairoError)
                })
                .map(FilterInput::StandardInput),

            Input::FillPaint => draw_ctx
                .get_paint_server_surface(
                    self.source_surface.width(),
                    self.source_surface.height(),
                    acquired_nodes,
                    &values.fill().0,
                    values.fill_opacity().0,
                    &self.node_bbox,
                    self.computed_from_node_being_filtered.color().0,
                    &values,
                )
                .map_err(FilterError::CairoError)
                .map(FilterInput::StandardInput),

            Input::StrokePaint => draw_ctx
                .get_paint_server_surface(
                    self.source_surface.width(),
                    self.source_surface.height(),
                    acquired_nodes,
                    &values.stroke().0,
                    values.stroke_opacity().0,
                    &self.node_bbox,
                    self.computed_from_node_being_filtered.color().0,
                    &values,
                )
                .map_err(FilterError::CairoError)
                .map(FilterInput::StandardInput),

            Input::FilterOutput(ref name) => self
                .previous_results
                .get(name)
                .cloned()
                .map(FilterInput::PrimitiveOutput)
                .ok_or(FilterError::InvalidInput),
        }
    }

    /// Retrieves the filter input surface according to the SVG rules.
    pub fn get_input(
        &self,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
        in_: Option<&Input>,
    ) -> Result<FilterInput, FilterError> {
        let raw = self.get_input_raw(acquired_nodes, draw_ctx, in_)?;

        // Convert the input surface to the desired format.
        let (surface, bounds) = match raw {
            FilterInput::StandardInput(ref surface) => {
                (surface, self.effects_region().rect.unwrap().into())
            }
            FilterInput::PrimitiveOutput(FilterOutput {
                ref surface,
                ref bounds,
            }) => (surface, *bounds),
        };

        let surface = if self.processing_linear_rgb {
            surface.to_linear_rgb(bounds)
        } else {
            surface.to_srgb(bounds)
        };
        surface
            .map_err(FilterError::CairoError)
            .map(|surface| match raw {
                FilterInput::StandardInput(_) => FilterInput::StandardInput(surface),
                FilterInput::PrimitiveOutput(ref output) => {
                    FilterInput::PrimitiveOutput(FilterOutput { surface, ..*output })
                }
            })
    }

    /// Calls the given closure with linear RGB processing enabled.
    #[inline]
    pub fn with_linear_rgb<T, F: FnOnce(&mut FilterContext) -> T>(&mut self, f: F) -> T {
        self.processing_linear_rgb = true;
        let rv = f(self);
        self.processing_linear_rgb = false;
        rv
    }

    /// Applies the `primitiveUnits` coordinate transformation to a non-x or y distance.
    #[inline]
    pub fn transform_dist(&self, d: f64) -> f64 {
        d * (self.paffine.xx.powi(2) + self.paffine.yy.powi(2)).sqrt() / f64::consts::SQRT_2
    }
}

impl FilterInput {
    /// Retrieves the surface from `FilterInput`.
    #[inline]
    pub fn surface(&self) -> &SharedImageSurface {
        match *self {
            FilterInput::StandardInput(ref surface) => surface,
            FilterInput::PrimitiveOutput(FilterOutput { ref surface, .. }) => surface,
        }
    }
}
