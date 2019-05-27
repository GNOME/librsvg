use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::f64;

use cairo::{self, MatrixTrait};

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{DrawingCtx, ViewParams};
use crate::node::RsvgNode;
use crate::paint_server::PaintServer;
use crate::properties::ComputedValues;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
use crate::unit_interval::UnitInterval;

use super::error::FilterError;
use super::input::Input;
use super::node::NodeFilter;

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
    pub name: Option<String>,

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
    node: RsvgNode,
    /// Bounding box of node being filtered
    node_bbox: BoundingBox,
    /// Values from the node which referenced this filter.
    computed_from_node_being_filtered: ComputedValues,
    /// The source graphic surface.
    source_surface: SharedImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<FilterOutput>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<String, FilterOutput>,
    /// The background surface. Computed lazily.
    background_surface: UnsafeCell<Option<Result<SharedImageSurface, FilterError>>>,
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
    _affine: cairo::Matrix,

    /// The filter primitive affine matrix.
    ///
    /// See the comments for `_affine`, they largely apply here.
    paffine: cairo::Matrix,
}

impl FilterContext {
    /// Creates a new `FilterContext`.
    pub fn new(
        filter_node: &RsvgNode,
        computed_from_node_being_filtered: &ComputedValues,
        source_surface: SharedImageSurface,
        draw_ctx: &mut DrawingCtx,
        node_bbox: BoundingBox,
    ) -> Self {
        let cr_affine = draw_ctx.get_cairo_context().get_matrix();

        // The rect can be empty (for example, if the filter is applied to an empty group).
        // However, with userSpaceOnUse it's still possible to create images with a filter.
        let bbox_rect = node_bbox.rect.unwrap_or(cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });

        let filter = filter_node.get_impl::<NodeFilter>();

        let affine = match filter.filterunits.get() {
            CoordUnits::UserSpaceOnUse => cr_affine,
            CoordUnits::ObjectBoundingBox => {
                let affine = cairo::Matrix::new(
                    bbox_rect.width,
                    0f64,
                    0f64,
                    bbox_rect.height,
                    bbox_rect.x,
                    bbox_rect.y,
                );
                cairo::Matrix::multiply(&affine, &cr_affine)
            }
        };

        let paffine = match filter.primitiveunits.get() {
            CoordUnits::UserSpaceOnUse => cr_affine,
            CoordUnits::ObjectBoundingBox => {
                let affine = cairo::Matrix::new(
                    bbox_rect.width,
                    0f64,
                    0f64,
                    bbox_rect.height,
                    bbox_rect.x,
                    bbox_rect.y,
                );
                cairo::Matrix::multiply(&affine, &cr_affine)
            }
        };

        let width = source_surface.width();
        let height = source_surface.height();

        Self {
            node: filter_node.clone(),
            node_bbox,
            computed_from_node_being_filtered: computed_from_node_being_filtered.clone(),
            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
            background_surface: UnsafeCell::new(None),
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

    /// Returns the surface corresponding to the last filter primitive's result.
    #[inline]
    pub fn last_result(&self) -> Option<&FilterOutput> {
        self.last_result.as_ref()
    }

    /// Returns the surface corresponding to the source graphic.
    #[inline]
    pub fn source_graphic(&self) -> &SharedImageSurface {
        &self.source_surface
    }

    /// Returns the surface containing the source graphic alpha.
    #[inline]
    pub fn source_alpha(&self, bounds: IRect) -> Result<SharedImageSurface, FilterError> {
        self.source_surface
            .extract_alpha(bounds)
            .map_err(FilterError::CairoError)
    }

    /// Returns the surface corresponding to the background image snapshot.
    pub fn background_image(
        &self,
        draw_ctx: &DrawingCtx,
    ) -> Result<&SharedImageSurface, FilterError> {
        {
            // At this point either no, or only immutable references to background_surface exist, so
            // it's ok to make an immutable reference.
            let bg = unsafe { &*self.background_surface.get() };

            // If background_surface was already computed, return the immutable reference. It will
            // get bound to the &self lifetime by the function return type.
            if let Some(result) = bg.as_ref() {
                return result.as_ref().map_err(|&s| s);
            }
        }

        // If we got here, then background_surface hasn't been computed yet. This means there are
        // no references to it and we can create a mutable reference.
        let bg = unsafe { &mut *self.background_surface.get() };

        *bg = Some(
            cairo::ImageSurface::create(
                cairo::Format::ARgb32,
                self.source_surface.width(),
                self.source_surface.height(),
            )
            .map_err(FilterError::CairoError)
            .and_then(|s| {
                draw_ctx.get_snapshot(&s);
                SharedImageSurface::new(s, SurfaceType::SRgb).map_err(FilterError::CairoError)
            }),
        );

        // Return the only existing reference as immutable.
        bg.as_ref().unwrap().as_ref().map_err(|&s| s)
    }

    /// Returns the surface containing the background image snapshot alpha.
    #[inline]
    pub fn background_alpha(
        &self,
        draw_ctx: &DrawingCtx,
        bounds: IRect,
    ) -> Result<SharedImageSurface, FilterError> {
        self.background_image(draw_ctx)?
            .extract_alpha(bounds)
            .map_err(FilterError::CairoError)
    }

    /// Returns the output of the filter primitive by its result name.
    #[inline]
    pub fn filter_output(&self, name: &str) -> Option<&FilterOutput> {
        self.previous_results.get(name)
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
            None => {
                let empty_surface = cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    self.source_surface.width(),
                    self.source_surface.height(),
                )?;

                Ok(SharedImageSurface::new(
                    empty_surface,
                    SurfaceType::AlphaOnly,
                )?)
            }
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
    pub fn paffine(&self) -> cairo::Matrix {
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
        let filter = self.node.get_impl::<NodeFilter>();

        // See comments in compute_effects_region() for how this works.
        if filter.primitiveunits.get() == CoordUnits::ObjectBoundingBox {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        }
    }

    /// Computes and returns a surface corresponding to the given paint server.
    fn get_paint_server_surface(
        &self,
        draw_ctx: &mut DrawingCtx,
        paint_server: &PaintServer,
        opacity: UnitInterval,
    ) -> Result<cairo::ImageSurface, cairo::Status> {
        let surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            self.source_surface.width(),
            self.source_surface.height(),
        )?;

        let cr_save = draw_ctx.get_cairo_context();
        let cr = cairo::Context::new(&surface);
        draw_ctx.set_cairo_context(&cr);

        // FIXME: we are ignoring the following error; propagate it upstream
        let _ = draw_ctx
            .set_source_paint_server(
                paint_server,
                &opacity,
                &self.node_bbox,
                &self.computed_from_node_being_filtered.color.0,
            )
            .and_then(|had_paint_server| {
                if had_paint_server {
                    cr.paint();
                }
                Ok(())
            });

        draw_ctx.set_cairo_context(&cr_save);
        Ok(surface)
    }

    /// Retrieves the filter input surface according to the SVG rules.
    ///
    /// Does not take `processing_linear_rgb` into account.
    fn get_input_raw(
        &self,
        draw_ctx: &mut DrawingCtx,
        in_: Option<&Input>,
    ) -> Result<FilterInput, FilterError> {
        if in_.is_none() {
            // No value => use the last result.
            // As per the SVG spec, if the filter primitive is the first in the chain, return the
            // source graphic.
            if let Some(output) = self.last_result().cloned() {
                return Ok(FilterInput::PrimitiveOutput(output));
            } else {
                return Ok(FilterInput::StandardInput(self.source_graphic().clone()));
            }
        }

        let values = &self.computed_from_node_being_filtered;

        match *in_.unwrap() {
            Input::SourceGraphic => Ok(FilterInput::StandardInput(self.source_graphic().clone())),
            Input::SourceAlpha => self
                .source_alpha(self.effects_region().rect.unwrap().into())
                .map(FilterInput::StandardInput),
            Input::BackgroundImage => self
                .background_image(draw_ctx)
                .map(Clone::clone)
                .map(FilterInput::StandardInput),
            Input::BackgroundAlpha => self
                .background_alpha(draw_ctx, self.effects_region().rect.unwrap().into())
                .map(FilterInput::StandardInput),

            Input::FillPaint => self
                .get_paint_server_surface(draw_ctx, &values.fill.0, values.fill_opacity.0)
                .map_err(FilterError::CairoError)
                .and_then(|surface| {
                    SharedImageSurface::new(surface, SurfaceType::SRgb)
                        .map_err(FilterError::CairoError)
                })
                .map(FilterInput::StandardInput),
            Input::StrokePaint => self
                .get_paint_server_surface(draw_ctx, &values.stroke.0, values.stroke_opacity.0)
                .map_err(FilterError::CairoError)
                .and_then(|surface| {
                    SharedImageSurface::new(surface, SurfaceType::SRgb)
                        .map_err(FilterError::CairoError)
                })
                .map(FilterInput::StandardInput),

            Input::FilterOutput(ref name) => self
                .filter_output(name)
                .cloned()
                .map(FilterInput::PrimitiveOutput)
                .ok_or(FilterError::InvalidInput),
        }
    }

    /// Retrieves the filter input surface according to the SVG rules.
    pub fn get_input(
        &self,
        draw_ctx: &mut DrawingCtx,
        in_: Option<&Input>,
    ) -> Result<FilterInput, FilterError> {
        let raw = self.get_input_raw(draw_ctx, in_)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_utils::iterators::Pixels;

    #[test]
    fn test_extract_alpha() {
        const WIDTH: i32 = 32;
        const HEIGHT: i32 = 64;
        const BOUNDS: IRect = IRect {
            x0: 8,
            x1: 24,
            y0: 16,
            y1: 48,
        };
        const FULL_BOUNDS: IRect = IRect {
            x0: 0,
            x1: WIDTH,
            y0: 0,
            y1: HEIGHT,
        };

        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, WIDTH, HEIGHT).unwrap();

        // Fill the surface with some data.
        {
            let mut data = surface.get_data().unwrap();

            let mut counter = 0u16;
            for x in data.iter_mut() {
                *x = counter as u8;
                counter = (counter + 1) % 256;
            }
        }

        let surface = SharedImageSurface::new(surface, SurfaceType::SRgb).unwrap();
        let alpha = surface.extract_alpha(BOUNDS).unwrap();

        for (x, y, p, pa) in
            Pixels::new(&surface, FULL_BOUNDS).map(|(x, y, p)| (x, y, p, alpha.get_pixel(x, y)))
        {
            assert_eq!(pa.r, 0);
            assert_eq!(pa.g, 0);
            assert_eq!(pa.b, 0);

            if !BOUNDS.contains(x as i32, y as i32) {
                assert_eq!(pa.a, 0);
            } else {
                assert_eq!(pa.a, p.a);
            }
        }
    }
}
