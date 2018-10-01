use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::f64;

use cairo::{self, MatrixTrait};

use bbox::BoundingBox;
use coord_units::CoordUnits;
use drawing_ctx::DrawingCtx;
use length::Length;
use node::RsvgNode;
use paint_server::{self, PaintServer};
use state::ComputedValues;
use surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
use unitinterval::UnitInterval;

use super::error::FilterError;
use super::input::Input;
use super::node::NodeFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

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

/// Computes and returns the filter effects region.
fn compute_effects_region(
    filter_node: &RsvgNode,
    computed_from_target_node: &ComputedValues,
    draw_ctx: &mut DrawingCtx<'_>,
    affine: cairo::Matrix,
    width: f64,
    height: f64,
) -> BoundingBox {
    // Filters use the properties of the target node.
    let values = computed_from_target_node;

    let filter = filter_node.get_impl::<NodeFilter>().unwrap();

    let mut bbox = BoundingBox::new(&cairo::Matrix::identity());

    // affine is set up in FilterContext::new() in such a way that for
    // filterunits == ObjectBoundingBox affine includes scaling to correct width, height and this
    // is why width and height are set to 1, 1 (and for filterunits == UserSpaceOnUse affine
    // doesn't include scaling because in this case the correct width, height already happens to be
    // the viewbox width, height).
    //
    // It's done this way because with ObjectBoundingBox, non-percentage values are supposed to
    // represent the fractions of the referenced node, and with width and height = 1, 1 this
    // works out exactly like that.
    let params = if filter.filterunits.get() == CoordUnits::ObjectBoundingBox {
        draw_ctx.push_view_box(1.0, 1.0)
    } else {
        draw_ctx.get_view_params()
    };

    // With filterunits == ObjectBoundingBox, lengths represent fractions or percentages of the
    // referencing node. No units are allowed (it's checked during attribute parsing).
    let rect = if filter.filterunits.get() == CoordUnits::ObjectBoundingBox {
        cairo::Rectangle {
            x: filter.x.get().get_unitless(),
            y: filter.y.get().get_unitless(),
            width: filter.width.get().get_unitless(),
            height: filter.height.get().get_unitless(),
        }
    } else {
        cairo::Rectangle {
            x: filter.x.get().normalize(values, &params),
            y: filter.y.get().normalize(values, &params),
            width: filter.width.get().normalize(values, &params),
            height: filter.height.get().normalize(values, &params),
        }
    };

    let other_bbox = BoundingBox::new(&affine).with_rect(Some(rect));

    // At this point all of the previous viewbox and matrix business gets converted to pixel
    // coordinates in the final surface, because bbox is created with an identity affine.
    bbox.insert(&other_bbox);

    // Finally, clip to the width and height of our surface.
    let rect = cairo::Rectangle {
        x: 0f64,
        y: 0f64,
        width,
        height,
    };
    let other_bbox = BoundingBox::new(&cairo::Matrix::identity()).with_rect(Some(rect));
    bbox.clip(&other_bbox);

    bbox
}

impl IRect {
    /// Returns true if the `IRect` contains the given coordinates.
    #[inline]
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    /// Returns an `IRect` scaled by the given amounts.
    ///
    /// The returned `IRect` encompasses all, even partially covered, pixels after the scaling.
    #[inline]
    pub fn scale(self, x: f64, y: f64) -> IRect {
        IRect {
            x0: (f64::from(self.x0) * x).floor() as i32,
            y0: (f64::from(self.y0) * y).floor() as i32,
            x1: (f64::from(self.x1) * x).ceil() as i32,
            y1: (f64::from(self.y1) * y).ceil() as i32,
        }
    }
}

impl FilterContext {
    /// Creates a new `FilterContext`.
    pub fn new(
        filter_node: &RsvgNode,
        computed_from_node_being_filtered: &ComputedValues,
        source_surface: SharedImageSurface,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Self {
        let cr_affine = draw_ctx.get_cairo_context().get_matrix();
        let bbox = draw_ctx.get_bbox().clone();

        // The rect can be empty (for example, if the filter is applied to an empty group).
        // However, with userSpaceOnUse it's still possible to create images with a filter.
        let bbox_rect = bbox.rect.unwrap_or(cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });

        let filter = filter_node.get_impl::<NodeFilter>().unwrap();

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
            computed_from_node_being_filtered: computed_from_node_being_filtered.clone(),
            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
            background_surface: UnsafeCell::new(None),
            effects_region: compute_effects_region(
                filter_node,
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

    /// Computes and returns the background image snapshot.
    fn compute_background_image(
        &self,
        draw_ctx: &DrawingCtx<'_>,
    ) -> Result<cairo::ImageSurface, cairo::Status> {
        let surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            self.source_surface.width(),
            self.source_surface.height(),
        )?;

        let (x, y) = draw_ctx.get_raw_offset();
        let stack = draw_ctx.get_cr_stack();

        // TODO: as far as I can tell this should not render elements past the last (topmost) one
        // with enable-background: new (because technically we shouldn't have been caching them).
        // Right now there are no enable-background checks whatsoever.
        let cr = cairo::Context::new(&surface);
        for draw in stack.into_iter() {
            let nested = draw_ctx.is_cairo_context_nested(&draw);
            cr.set_source_surface(
                &draw.get_target(),
                if nested { 0f64 } else { -x },
                if nested { 0f64 } else { -y },
            );
            cr.paint();
        }

        Ok(surface)
    }

    /// Returns the surface corresponding to the background image snapshot.
    pub fn background_image(
        &self,
        draw_ctx: &DrawingCtx<'_>,
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
            self.compute_background_image(draw_ctx)
                .map_err(FilterError::CairoError)
                .and_then(|surface| {
                    SharedImageSurface::new(surface, SurfaceType::SRgb)
                        .map_err(FilterError::CairoError)
                }),
        );

        // Return the only existing reference as immutable.
        bg.as_ref().unwrap().as_ref().map_err(|&s| s)
    }

    /// Returns the surface containing the background image snapshot alpha.
    #[inline]
    pub fn background_alpha(
        &self,
        draw_ctx: &DrawingCtx<'_>,
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

    /// Calls the given function with correct behavior for the value of `primitiveUnits`.
    pub fn with_primitive_units<F, T>(&self, draw_ctx: &mut DrawingCtx<'_>, f: F) -> T
    // TODO: Get rid of this Box? Can't just impl Trait because Rust cannot do higher-ranked types.
    where
        for<'b> F: FnOnce(Box<Fn(&Length) -> f64 + 'b>) -> T,
    {
        let filter = self.node.get_impl::<NodeFilter>().unwrap();

        // See comments in compute_effects_region() for how this works.
        if filter.primitiveunits.get() == CoordUnits::ObjectBoundingBox {
            let _params = draw_ctx.push_view_box(1.0, 1.0);
            let rv = f(Box::new(Length::get_unitless));

            rv
        } else {
            f(Box::new(|length: &Length| {
                // Filters use the properties of the target node.
                length.normalize(
                    &self.computed_from_node_being_filtered,
                    &draw_ctx.get_view_params(),
                )
            }))
        }
    }

    /// Computes and returns a surface corresponding to the given paint server.
    fn get_paint_server_surface(
        &self,
        draw_ctx: &mut DrawingCtx<'_>,
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

        let bbox = draw_ctx.get_bbox().clone();

        // FIXME: we are ignoring the following error; propagate it upstream
        let _ = paint_server::set_source_paint_server(
            draw_ctx,
            paint_server,
            &opacity,
            &bbox,
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
        draw_ctx: &mut DrawingCtx<'_>,
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
        draw_ctx: &mut DrawingCtx<'_>,
        in_: Option<&Input>,
    ) -> Result<FilterInput, FilterError> {
        let raw = self.get_input_raw(draw_ctx, in_)?;

        // Linearize the returned surface if needed.
        if self.processing_linear_rgb {
            let (surface, bounds) = match raw {
                FilterInput::StandardInput(ref surface) => {
                    (surface, self.effects_region().rect.unwrap().into())
                }
                FilterInput::PrimitiveOutput(FilterOutput {
                    ref surface,
                    ref bounds,
                }) => (surface, *bounds),
            };

            surface
                .to_linear_rgb(bounds)
                .map_err(FilterError::CairoError)
                .map(|surface| match raw {
                    FilterInput::StandardInput(_) => FilterInput::StandardInput(surface),
                    FilterInput::PrimitiveOutput(ref output) => {
                        FilterInput::PrimitiveOutput(FilterOutput { surface, ..*output })
                    }
                })
        } else {
            Ok(raw)
        }
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

impl From<cairo::Rectangle> for IRect {
    #[inline]
    fn from(
        cairo::Rectangle {
            x,
            y,
            width,
            height,
        }: cairo::Rectangle,
    ) -> Self {
        Self {
            x0: x.floor() as i32,
            y0: y.floor() as i32,
            x1: (x + width).ceil() as i32,
            y1: (y + height).ceil() as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use surface_utils::iterators::Pixels;

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
