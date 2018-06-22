use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::{mem, ptr};

use cairo::prelude::SurfaceExt;
use cairo::{self, MatrixTrait};
use cairo_sys::cairo_surface_t;
use glib::translate::{from_glib_full, from_glib_none};
use glib_sys::*;

use bbox::BoundingBox;
use coord_units::CoordUnits;
use drawing_ctx::{self, RsvgDrawingCtx};
use length::RsvgLength;
use node::{box_node, RsvgNode};
use paint_server::{self, PaintServer};
use srgb::{linearize_surface, unlinearize_surface};
use surface_utils::{iterators::Pixels, shared_surface::SharedImageSurface, Pixel};
use unitinterval::UnitInterval;

use super::bounds::BoundsBuilder;
use super::input::Input;
use super::node::NodeFilter;
use super::RsvgFilterPrimitive;

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/rsvg-filter.h:RsvgIRect
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterPrimitiveOutput
#[repr(C)]
pub struct RsvgFilterPrimitiveOutput {
    surface: *mut cairo_surface_t,
    bounds: IRect,
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

pub type RsvgFilterContext = FilterContext;

/// The filter rendering context.
pub struct FilterContext {
    /// The <filter> node.
    node: RsvgNode,
    /// The node which referenced this filter.
    node_being_filtered: RsvgNode,
    /// The source graphic surface.
    source_surface: SharedImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<FilterOutput>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<String, FilterOutput>,
    /// The background surface. Computed lazily.
    background_surface: UnsafeCell<Option<Result<SharedImageSurface, cairo::Status>>>,
    /// The drawing context.
    drawing_ctx: *mut RsvgDrawingCtx,
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
    affine: cairo::Matrix,

    /// The filter primitive affine matrix.
    ///
    /// See the comments for `affine`, they largely apply here.
    paffine: cairo::Matrix,

    /// Obsolete; remove when all filters are ported to Rust.
    channelmap: [i32; 4],
}

/// Returns a surface with black background and alpha channel matching the input surface.
fn extract_alpha(
    surface: &SharedImageSurface,
    bounds: IRect,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let mut output_surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, surface.width(), surface.height())?;

    let output_stride = output_surface.get_stride() as usize;
    {
        let mut output_data = output_surface.get_data().unwrap();

        for (x, y, Pixel { a, .. }) in Pixels::new(surface, bounds) {
            output_data[y as usize * output_stride + x as usize * 4 + 3] = a;
        }
    }

    Ok(output_surface)
}

/// Computes and returns the filter effects region.
fn compute_effects_region(
    filter_node: &RsvgNode,
    target_node: &RsvgNode,
    drawing_ctx: *mut RsvgDrawingCtx,
    affine: cairo::Matrix,
    width: f64,
    height: f64,
) -> BoundingBox {
    // Filters use the properties of the target node.
    let cascaded = target_node.get_cascaded_values();
    let values = cascaded.get();

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
    if filter.filterunits.get() == CoordUnits::ObjectBoundingBox {
        drawing_ctx::push_view_box(drawing_ctx, 1f64, 1f64);
    }

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
            x: filter.x.get().normalize(values, drawing_ctx),
            y: filter.y.get().normalize(values, drawing_ctx),
            width: filter.width.get().normalize(values, drawing_ctx),
            height: filter.height.get().normalize(values, drawing_ctx),
        }
    };

    if filter.filterunits.get() == CoordUnits::ObjectBoundingBox {
        drawing_ctx::pop_view_box(drawing_ctx);
    }

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
}

impl FilterContext {
    /// Creates a new `FilterContext`.
    pub fn new(
        filter_node: &RsvgNode,
        node_being_filtered: &RsvgNode,
        source_surface: SharedImageSurface,
        draw_ctx: *mut RsvgDrawingCtx,
        channelmap: [i32; 4],
    ) -> Self {
        let cr_affine = drawing_ctx::get_cairo_context(draw_ctx).get_matrix();
        let bbox = drawing_ctx::get_bbox(draw_ctx);
        let bbox_rect = bbox.rect.unwrap();

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

        let mut rv = Self {
            node: filter_node.clone(),
            node_being_filtered: node_being_filtered.clone(),
            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
            background_surface: UnsafeCell::new(None),
            drawing_ctx: draw_ctx,
            effects_region: compute_effects_region(
                filter_node,
                node_being_filtered,
                draw_ctx,
                affine,
                f64::from(width),
                f64::from(height),
            ),
            processing_linear_rgb: false,
            affine,
            paffine,
            channelmap,
        };

        let last_result = FilterOutput {
            surface: rv.source_surface.clone(),
            bounds: rv.effects_region().rect.unwrap().into(),
        };

        rv.last_result = Some(last_result);
        rv
    }

    /// Returns the <filter> node for this context.
    #[inline]
    pub fn get_filter_node(&self) -> RsvgNode {
        self.node.clone()
    }

    /// Returns the node that referenced this filter.
    #[inline]
    pub fn get_node_being_filtered(&self) -> RsvgNode {
        self.node_being_filtered.clone()
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
    pub fn source_alpha(&self, bounds: IRect) -> Result<cairo::ImageSurface, cairo::Status> {
        extract_alpha(self.source_graphic(), bounds)
    }

    /// Computes and returns the background image snapshot.
    fn compute_background_image(&self) -> Result<cairo::ImageSurface, cairo::Status> {
        let surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            self.source_surface.width(),
            self.source_surface.height(),
        )?;

        let (x, y) = drawing_ctx::get_raw_offset(self.drawing_ctx);
        let stack = drawing_ctx::get_cr_stack(self.drawing_ctx);

        let cr = cairo::Context::new(&surface);
        for draw in stack.into_iter().rev() {
            let nested = drawing_ctx::is_cairo_context_nested(self.drawing_ctx, &draw);
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
    pub fn background_image(&self) -> Result<&SharedImageSurface, cairo::Status> {
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
            self.compute_background_image()
                .and_then(SharedImageSurface::new),
        );

        // Return the only existing reference as immutable.
        bg.as_ref().unwrap().as_ref().map_err(|&s| s)
    }

    /// Returns the surface containing the background image snapshot alpha.
    #[inline]
    pub fn background_alpha(&self, bounds: IRect) -> Result<cairo::ImageSurface, cairo::Status> {
        self.background_image()
            .and_then(|surface| extract_alpha(surface, bounds))
    }

    /// Returns the output of the filter primitive by its result name.
    #[inline]
    pub fn filter_output(&self, name: &str) -> Option<&FilterOutput> {
        self.previous_results.get(name)
    }

    /// Converts this `FilterContext` into the surface corresponding to the output of the filter
    /// chain.
    #[inline]
    pub fn into_output(self) -> SharedImageSurface {
        self.last_result
            .map(|FilterOutput { surface, .. }| surface)
            .unwrap_or(self.source_surface)
    }

    /// Stores a filter primitive result into the context.
    #[inline]
    pub fn store_result(&mut self, mut result: FilterResult) {
        // Unlinearize the surface if needed.
        if self.processing_linear_rgb {
            // TODO: unwrap() on unlinearize_surface() can panic if we run out of memory for Cairo.
            result.output.surface = SharedImageSurface::new(
                unlinearize_surface(&result.output.surface, result.output.bounds).unwrap(),
            ).unwrap();
        }

        if let Some(name) = result.name {
            self.previous_results.insert(name, result.output.clone());
        }

        self.last_result = Some(result.output);
    }

    /// Returns the drawing context for this filter context.
    #[inline]
    pub fn drawing_context(&self) -> *mut RsvgDrawingCtx {
        self.drawing_ctx
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
    pub fn with_primitive_units<F, T>(&self, f: F) -> T
    // TODO: Get rid of this Box? Can't just impl Trait because Rust cannot do higher-ranked types.
    where
        for<'a> F: FnOnce(Box<Fn(&RsvgLength) -> f64 + 'a>) -> T,
    {
        // Filters use the properties of the target node.
        let cascaded = self.node_being_filtered.get_cascaded_values();
        let values = cascaded.get();

        let filter = self.node.get_impl::<NodeFilter>().unwrap();

        // See comments in compute_effects_region() for how this works.
        if filter.primitiveunits.get() == CoordUnits::ObjectBoundingBox {
            drawing_ctx::push_view_box(self.drawing_ctx, 1f64, 1f64);

            let rv = f(Box::new(RsvgLength::get_unitless));

            drawing_ctx::pop_view_box(self.drawing_ctx);

            rv
        } else {
            f(Box::new(|length: &RsvgLength| {
                length.normalize(values, self.drawing_ctx)
            }))
        }
    }

    /// Computes and returns a surface corresponding to the given paint server.
    fn get_paint_server_surface(
        &self,
        paint_server: &PaintServer,
        opacity: UnitInterval,
    ) -> Result<cairo::ImageSurface, cairo::Status> {
        let surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            self.source_surface.width(),
            self.source_surface.height(),
        )?;

        let cr_save = drawing_ctx::get_cairo_context(self.drawing_ctx);
        let cr = cairo::Context::new(&surface);
        drawing_ctx::set_cairo_context(self.drawing_ctx, &cr);

        let cascaded = self.node_being_filtered.get_cascaded_values();
        let values = cascaded.get();

        if paint_server::set_source_paint_server(
            self.drawing_ctx,
            paint_server,
            &opacity,
            drawing_ctx::get_bbox(self.drawing_ctx),
            &values.color.0,
        ) {
            cr.paint();
        }

        drawing_ctx::set_cairo_context(self.drawing_ctx, &cr_save);
        Ok(surface)
    }

    /// Retrieves the filter input surface according to the SVG rules.
    ///
    /// Does not take `processing_linear_rgb` into account.
    // TODO: pass the errors through.
    fn get_input_raw(&self, in_: Option<&Input>) -> Option<FilterInput> {
        if in_.is_none() {
            // No value => use the last result.
            // As per the SVG spec, if the filter primitive is the first in the chain, return the
            // source graphic.
            if let Some(output) = self.last_result().cloned() {
                return Some(FilterInput::PrimitiveOutput(output));
            } else {
                return Some(FilterInput::StandardInput(self.source_graphic().clone()));
            }
        }

        let cascaded = self.node_being_filtered.get_cascaded_values();
        let values = cascaded.get();

        match *in_.unwrap() {
            Input::SourceGraphic => Some(FilterInput::StandardInput(self.source_graphic().clone())),
            Input::SourceAlpha => self
                .source_alpha(self.effects_region().rect.unwrap().into())
                .ok()
                .map(|surface| SharedImageSurface::new(surface).unwrap())
                .map(FilterInput::StandardInput),
            Input::BackgroundImage => self
                .background_image()
                .ok()
                .cloned()
                .map(FilterInput::StandardInput),
            Input::BackgroundAlpha => self
                .background_alpha(self.effects_region().rect.unwrap().into())
                .ok()
                .map(|surface| SharedImageSurface::new(surface).unwrap())
                .map(FilterInput::StandardInput),

            Input::FillPaint => self
                .get_paint_server_surface(&values.fill.0, values.fill_opacity.0)
                .ok()
                .map(|surface| SharedImageSurface::new(surface).unwrap())
                .map(FilterInput::StandardInput),
            Input::StrokePaint => self
                .get_paint_server_surface(&values.stroke.0, values.stroke_opacity.0)
                .ok()
                .map(|surface| SharedImageSurface::new(surface).unwrap())
                .map(FilterInput::StandardInput),

            Input::FilterOutput(ref name) => self
                .filter_output(name)
                .cloned()
                .map(FilterInput::PrimitiveOutput),
        }
    }

    /// Retrieves the filter input surface according to the SVG rules.
    pub fn get_input(&self, in_: Option<&Input>) -> Option<FilterInput> {
        let raw = self.get_input_raw(in_);

        // Linearize the returned surface if needed.
        if raw.is_some() && self.processing_linear_rgb {
            let (surface, bounds) = match raw.as_ref().unwrap() {
                FilterInput::StandardInput(ref surface) => {
                    (surface, self.effects_region().rect.unwrap().into())
                }
                FilterInput::PrimitiveOutput(FilterOutput {
                    ref surface,
                    ref bounds,
                }) => (surface, *bounds),
            };

            linearize_surface(surface, bounds)
                .ok()
                .map(|surface| SharedImageSurface::new(surface).unwrap())
                .map(|surface| match raw.as_ref().unwrap() {
                    FilterInput::StandardInput(_) => FilterInput::StandardInput(surface),
                    FilterInput::PrimitiveOutput(output) => {
                        FilterInput::PrimitiveOutput(FilterOutput { surface, ..*output })
                    }
                })
        } else {
            raw
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

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_affine(
    ctx: *const RsvgFilterContext,
) -> cairo::Matrix {
    assert!(!ctx.is_null());

    (*ctx).affine
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_paffine(
    ctx: *const RsvgFilterContext,
) -> cairo::Matrix {
    assert!(!ctx.is_null());

    (*ctx).paffine
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_drawing_ctx(
    ctx: *mut RsvgFilterContext,
) -> *mut RsvgDrawingCtx {
    assert!(!ctx.is_null());

    (*ctx).drawing_ctx
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_width(ctx: *const RsvgFilterContext) -> i32 {
    assert!(!ctx.is_null());

    (*ctx).source_surface.width()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_height(ctx: *const RsvgFilterContext) -> i32 {
    assert!(!ctx.is_null());

    (*ctx).source_surface.height()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_node_being_filtered(
    ctx: *const RsvgFilterContext,
) -> *mut RsvgNode {
    assert!(!ctx.is_null());

    box_node((*ctx).get_node_being_filtered())
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_channelmap(
    ctx: *const RsvgFilterContext,
) -> *const i32 {
    assert!(!ctx.is_null());

    (*ctx).channelmap.as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_source_surface(
    ctx: *mut RsvgFilterContext,
) -> *mut cairo_surface_t {
    assert!(!ctx.is_null());

    (*ctx).source_surface.to_glib_none().0
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_bg_surface(
    ctx: *mut RsvgFilterContext,
) -> *mut cairo_surface_t {
    assert!(!ctx.is_null());

    (*ctx)
        .background_image()
        .map(|surface| surface.to_glib_none().0)
        .unwrap_or_else(|_| ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_lastresult(
    ctx: *mut RsvgFilterContext,
) -> RsvgFilterPrimitiveOutput {
    assert!(!ctx.is_null());

    let ctx = &*ctx;

    match ctx.last_result {
        Some(FilterOutput {
            ref surface,
            ref bounds,
        }) => RsvgFilterPrimitiveOutput {
            surface: surface.to_glib_none().0,
            bounds: *bounds,
        },
        None => RsvgFilterPrimitiveOutput {
            surface: ctx.source_surface.to_glib_none().0,
            bounds: ctx.effects_region().rect.unwrap().into(),
        },
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_previous_result(
    name: *mut GString,
    ctx: *mut RsvgFilterContext,
    output: *mut RsvgFilterPrimitiveOutput,
) -> i32 {
    assert!(!name.is_null());
    assert!(!ctx.is_null());
    assert!(!output.is_null());

    if let Some(&FilterOutput {
        ref surface,
        ref bounds,
    }) = (*ctx).filter_output(&CStr::from_ptr((*name).str).to_string_lossy())
    {
        *output = RsvgFilterPrimitiveOutput {
            surface: surface.to_glib_none().0,
            bounds: *bounds,
        };
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_store_output(
    name: *mut GString,
    result: RsvgFilterPrimitiveOutput,
    ctx: *mut RsvgFilterContext,
) {
    assert!(!name.is_null());
    assert!(!result.surface.is_null());
    assert!(!ctx.is_null());

    let name = from_glib_none((*name).str);

    let surface: cairo::Surface = from_glib_full(result.surface);
    assert_eq!(surface.get_type(), cairo::SurfaceType::Image);
    let surface = cairo::ImageSurface::from(surface).unwrap();
    let surface = SharedImageSurface::new(surface).unwrap();

    let result = FilterResult {
        name: Some(name),
        output: FilterOutput {
            surface,
            bounds: result.bounds,
        },
    };

    (*ctx).store_result(result);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_primitive_get_bounds(
    primitive: *const RsvgFilterPrimitive,
    ctx: *const RsvgFilterContext,
) -> IRect {
    assert!(!ctx.is_null());

    let ctx = &*ctx;

    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;

    if !primitive.is_null() {
        if (*primitive).x_specified != 0 {
            x = Some((*primitive).x)
        };

        if (*primitive).y_specified != 0 {
            y = Some((*primitive).y)
        };

        if (*primitive).width_specified != 0 {
            width = Some((*primitive).width)
        };

        if (*primitive).height_specified != 0 {
            height = Some((*primitive).height)
        };
    }

    // Doesn't take referenced nodes into account, which is wrong.
    BoundsBuilder::new(ctx, x, y, width, height).into_irect()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_get_result(
    name: *const GString,
    ctx: *const RsvgFilterContext,
) -> RsvgFilterPrimitiveOutput {
    assert!(!name.is_null());
    assert!(!ctx.is_null());

    let name: String = from_glib_none((*name).str);
    let input = match &name[..] {
        "" | "none" => None,
        "SourceGraphic" => Some(Input::SourceGraphic),
        "SourceAlpha" => Some(Input::SourceAlpha),
        "BackgroundImage" => Some(Input::BackgroundImage),
        "BackgroundAlpha" => Some(Input::BackgroundAlpha),
        "FillPaint" => Some(Input::FillPaint),
        "StrokePaint" => Some(Input::StrokePaint),
        _ => Some(Input::FilterOutput(name)),
    };

    let ctx = &*ctx;

    match ctx.get_input(input.as_ref()) {
        None => RsvgFilterPrimitiveOutput {
            surface: ptr::null_mut(),
            bounds: IRect {
                x0: 0,
                x1: 0,
                y0: 0,
                y1: 0,
            },
        },
        Some(input) => {
            // HACK because to_glib_full() is unimplemented!() on ImageSurface.
            let ptr = input.surface().to_glib_none().0;

            let rv = RsvgFilterPrimitiveOutput {
                surface: ptr,
                bounds: match input {
                    FilterInput::StandardInput(_) => ctx.effects_region().rect.unwrap().into(),
                    FilterInput::PrimitiveOutput(FilterOutput { bounds, .. }) => bounds,
                },
            };

            mem::forget(input);

            rv
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_get_in(
    name: *const GString,
    ctx: *const RsvgFilterContext,
) -> *mut cairo_surface_t {
    rsvg_filter_get_result(name, ctx).surface
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let surface = SharedImageSurface::new(surface).unwrap();
        let alpha = SharedImageSurface::new(extract_alpha(&surface, BOUNDS).unwrap()).unwrap();

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
