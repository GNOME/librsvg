use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::filter::UserSpaceFilter;
use crate::paint_server::UserSpacePaintSource;
use crate::parsers::CustomIdent;
use crate::properties::ColorInterpolationFilters;
use crate::rect::{IRect, Rect};
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
    /// Paint source for primitives which have an input value equal to `StrokePaint`.
    stroke_paint: Rc<UserSpacePaintSource>,
    /// Paint source for primitives which have an input value equal to `FillPaint`.
    fill_paint: Rc<UserSpacePaintSource>,

    /// The source graphic surface.
    source_surface: SharedImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<FilterOutput>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<CustomIdent, FilterOutput>,

    /// Input surface for primitives that require an input of `BackgroundImage` or `BackgroundAlpha`. Computed lazily.
    background_surface: OnceCell<Result<SharedImageSurface, FilterError>>,

    // Input surface for primitives that require an input of `StrokePaint`, Computed lazily.
    stroke_paint_surface: OnceCell<Result<SharedImageSurface, FilterError>>,

    // Input surface for primitives that require an input of `FillPaint`, Computed lazily.
    fill_paint_surface: OnceCell<Result<SharedImageSurface, FilterError>>,

    /// Primtive units
    primitive_units: CoordUnits,
    /// The filter effects region.
    effects_region: Rect,

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
        filter: &UserSpaceFilter,
        stroke_paint: Rc<UserSpacePaintSource>,
        fill_paint: Rc<UserSpacePaintSource>,
        source_surface: &SharedImageSurface,
        draw_transform: Transform,
        node_bbox: BoundingBox,
    ) -> Result<Self, FilterError> {
        // The rect can be empty (for example, if the filter is applied to an empty group).
        // However, with userSpaceOnUse it's still possible to create images with a filter.
        let bbox_rect = node_bbox.rect.unwrap_or_default();

        let affine = match filter.filter_units {
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

        let paffine = match filter.primitive_units {
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

        if !(affine.is_invertible() && paffine.is_invertible()) {
            return Err(FilterError::InvalidParameter(
                "transform is not invertible".to_string(),
            ));
        }

        let effects_region = {
            let mut bbox = BoundingBox::new();
            let other_bbox = BoundingBox::new()
                .with_transform(affine)
                .with_rect(filter.rect);

            // At this point all of the previous viewbox and matrix business gets converted to pixel
            // coordinates in the final surface, because bbox is created with an identity transform.
            bbox.insert(&other_bbox);

            // Finally, clip to the width and height of our surface.
            let (width, height) = (source_surface.width(), source_surface.height());
            let rect = Rect::from_size(f64::from(width), f64::from(height));
            let other_bbox = BoundingBox::new().with_rect(rect);
            bbox.clip(&other_bbox);

            bbox.rect.unwrap()
        };

        Ok(Self {
            stroke_paint,
            fill_paint,
            source_surface: source_surface.clone(),
            last_result: None,
            previous_results: HashMap::new(),
            background_surface: OnceCell::new(),
            stroke_paint_surface: OnceCell::new(),
            fill_paint_surface: OnceCell::new(),
            primitive_units: filter.primitive_units,
            effects_region,
            _affine: affine,
            paffine,
        })
    }

    /// Returns the surface corresponding to the source graphic.
    #[inline]
    pub fn source_graphic(&self) -> &SharedImageSurface {
        &self.source_surface
    }

    /// Returns the surface corresponding to the background image snapshot.
    fn background_image(&self, draw_ctx: &DrawingCtx) -> Result<SharedImageSurface, FilterError> {
        let res = self.background_surface.get_or_init(|| {
            draw_ctx
                .get_snapshot(self.source_surface.width(), self.source_surface.height())
                .map_err(FilterError::CairoError)
        });

        res.as_ref().map(|s| s.clone()).map_err(|e| e.clone())
    }

    /// Returns a surface filled with the current stroke's paint, for `StrokePaint` inputs in primitives.
    ///
    /// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#attr-valuedef-in-strokepaint>
    fn stroke_paint_image(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<SharedImageSurface, FilterError> {
        let res = self.stroke_paint_surface.get_or_init(|| {
            Ok(draw_ctx.get_paint_source_surface(
                self.source_surface.width(),
                self.source_surface.height(),
                acquired_nodes,
                &self.stroke_paint,
            )?)
        });

        res.as_ref().map(|s| s.clone()).map_err(|e| e.clone())
    }

    /// Returns a surface filled with the current fill's paint, for `FillPaint` inputs in primitives.
    ///
    /// Filter Effects 1: <https://www.w3.org/TR/filter-effects/#attr-valuedef-in-fillpaint>
    fn fill_paint_image(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<SharedImageSurface, FilterError> {
        let res = self.fill_paint_surface.get_or_init(|| {
            Ok(draw_ctx.get_paint_source_surface(
                self.source_surface.width(),
                self.source_surface.height(),
                acquired_nodes,
                &self.fill_paint,
            )?)
        });

        res.as_ref().map(|s| s.clone()).map_err(|e| e.clone())
    }

    /// Converts this `FilterContext` into the surface corresponding to the output of the filter
    /// chain.
    ///
    /// The returned surface is in the sRGB color space.
    // TODO: sRGB conversion should probably be done by the caller.
    #[inline]
    pub fn into_output(self) -> Result<SharedImageSurface, cairo::Error> {
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
    pub fn store_result(&mut self, result: FilterResult) {
        if let Some(name) = result.name {
            self.previous_results.insert(name, result.output.clone());
        }

        self.last_result = Some(result.output);
    }

    /// Returns the paffine matrix.
    #[inline]
    pub fn paffine(&self) -> Transform {
        self.paffine
    }

    /// Returns the primitive units.
    #[inline]
    pub fn primitive_units(&self) -> CoordUnits {
        self.primitive_units
    }

    /// Returns the filter effects region.
    #[inline]
    pub fn effects_region(&self) -> Rect {
        self.effects_region
    }

    /// Retrieves the filter input surface according to the SVG rules.
    fn get_input_raw(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
        in_: &Input,
    ) -> Result<FilterInput, FilterError> {
        match *in_ {
            Input::Unspecified => {
                // No value => use the last result.
                // As per the SVG spec, if the filter primitive is the first in the chain, return the
                // source graphic.
                if let Some(output) = self.last_result.as_ref() {
                    Ok(FilterInput::PrimitiveOutput(output.clone()))
                } else {
                    Ok(FilterInput::StandardInput(self.source_graphic().clone()))
                }
            }

            Input::SourceGraphic => Ok(FilterInput::StandardInput(self.source_graphic().clone())),

            Input::SourceAlpha => self
                .source_graphic()
                .extract_alpha(self.effects_region().into())
                .map_err(FilterError::CairoError)
                .map(FilterInput::StandardInput),

            Input::BackgroundImage => self
                .background_image(draw_ctx)
                .map(FilterInput::StandardInput),

            Input::BackgroundAlpha => self
                .background_image(draw_ctx)
                .and_then(|surface| {
                    surface
                        .extract_alpha(self.effects_region().into())
                        .map_err(FilterError::CairoError)
                })
                .map(FilterInput::StandardInput),

            Input::FillPaint => self
                .fill_paint_image(acquired_nodes, draw_ctx)
                .map(FilterInput::StandardInput),

            Input::StrokePaint => self
                .stroke_paint_image(acquired_nodes, draw_ctx)
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
    ///
    /// The surface will be converted to the color space specified by `color_interpolation_filters`.
    pub fn get_input(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
        in_: &Input,
        color_interpolation_filters: ColorInterpolationFilters,
    ) -> Result<FilterInput, FilterError> {
        let raw = self.get_input_raw(acquired_nodes, draw_ctx, in_)?;

        // Convert the input surface to the desired format.
        let (surface, bounds) = match raw {
            FilterInput::StandardInput(ref surface) => (surface, self.effects_region().into()),
            FilterInput::PrimitiveOutput(FilterOutput {
                ref surface,
                ref bounds,
            }) => (surface, *bounds),
        };

        let surface = match color_interpolation_filters {
            ColorInterpolationFilters::Auto => Ok(surface.clone()),
            ColorInterpolationFilters::LinearRgb => surface.to_linear_rgb(bounds),
            ColorInterpolationFilters::Srgb => surface.to_srgb(bounds),
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
