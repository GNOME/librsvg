use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::{mem, ptr};

use cairo::prelude::SurfaceExt;
use cairo::{self, MatrixTrait};
use cairo_sys::cairo_surface_t;
use glib::translate::{from_glib_none, ToGlibPtr};
use glib_sys::*;

use bbox::BoundingBox;
use coord_units::CoordUnits;
use drawing_ctx::{self, RsvgDrawingCtx};
use length::RsvgLength;
use node::{box_node, RsvgNode};

use super::input::Input;
use super::iterators::{ImageSurfaceDataShared, Pixel, Pixels};
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
    pub surface: cairo::ImageSurface,

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

pub type RsvgFilterContext = FilterContext;

/// The filter rendering context.
pub struct FilterContext {
    /// The <filter> node.
    node: RsvgNode,
    /// The node which referenced this filter
    node_being_filtered: RsvgNode,
    /// The source graphic surface.
    source_surface: cairo::ImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<FilterOutput>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<String, FilterOutput>,
    /// The background surface. Computed lazily.
    background_surface: UnsafeCell<Option<Result<cairo::ImageSurface, cairo::Status>>>,

    affine: cairo::Matrix,
    paffine: cairo::Matrix,
    drawing_ctx: *mut RsvgDrawingCtx,
    channelmap: [i32; 4],
}

/// Returns a surface with black background and alpha channel matching the input surface.
fn extract_alpha(
    surface: &cairo::ImageSurface,
    bounds: IRect,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let data = ImageSurfaceDataShared::new(surface).unwrap();

    let mut output_surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, data.width as i32, data.height as i32)?;

    let output_stride = output_surface.get_stride() as usize;
    {
        let mut output_data = output_surface.get_data().unwrap();

        for (x, y, Pixel { a, .. }) in Pixels::new(data, bounds) {
            output_data[y * output_stride + x * 4 + 3] = a;
        }
    }

    Ok(output_surface)
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
        source_surface: cairo::ImageSurface,
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

        let mut rv = Self {
            node: filter_node.clone(),
            node_being_filtered: node_being_filtered.clone(),
            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
            background_surface: UnsafeCell::new(None),
            affine,
            paffine,
            drawing_ctx: draw_ctx,
            channelmap,
        };

        let last_result = FilterOutput {
            surface: rv.source_surface.clone(),
            bounds: rv.compute_bounds(None, None, None, None),
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
    pub fn source_graphic(&self) -> &cairo::ImageSurface {
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
            self.source_surface.get_width(),
            self.source_surface.get_height(),
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
    pub fn background_image(&self) -> Result<&cairo::ImageSurface, cairo::Status> {
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

        *bg = Some(self.compute_background_image());

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
    pub fn into_output(self) -> cairo::ImageSurface {
        self.last_result
            .map(|FilterOutput { surface, .. }| surface)
            .unwrap_or(self.source_surface)
    }

    /// Stores a filter primitive result into the context.
    #[inline]
    pub fn store_result(&mut self, result: FilterResult) {
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

    /// Computes and returns the filter primitive bounds.
    pub fn compute_bounds(
        &self,
        x: Option<RsvgLength>,
        y: Option<RsvgLength>,
        width: Option<RsvgLength>,
        height: Option<RsvgLength>,
    ) -> IRect {
        let cascaded = self.node.get_cascaded_values();
        let values = cascaded.get();

        let filter = self.node.get_impl::<NodeFilter>().unwrap();
        let mut bbox = BoundingBox::new(&cairo::Matrix::identity());

        if filter.filterunits.get() == CoordUnits::ObjectBoundingBox {
            drawing_ctx::push_view_box(self.drawing_ctx, 1f64, 1f64);
        }

        let rect = cairo::Rectangle {
            x: filter.x.get().normalize(values, self.drawing_ctx),
            y: filter.y.get().normalize(values, self.drawing_ctx),
            width: filter.width.get().normalize(values, self.drawing_ctx),
            height: filter.height.get().normalize(values, self.drawing_ctx),
        };

        if filter.filterunits.get() == CoordUnits::ObjectBoundingBox {
            drawing_ctx::pop_view_box(self.drawing_ctx);
        }

        let other_bbox = BoundingBox::new(&self.affine).with_rect(Some(rect));
        bbox.insert(&other_bbox);

        if x.is_some() || y.is_some() || width.is_some() || height.is_some() {
            if filter.primitiveunits.get() == CoordUnits::ObjectBoundingBox {
                drawing_ctx::push_view_box(self.drawing_ctx, 1f64, 1f64);
            }

            let mut rect = cairo::Rectangle {
                x: x.map(|x| x.normalize(values, self.drawing_ctx))
                    .unwrap_or(0f64),
                y: y.map(|y| y.normalize(values, self.drawing_ctx))
                    .unwrap_or(0f64),
                ..rect
            };

            if width.is_some() || height.is_some() {
                let (vbox_width, vbox_height) = drawing_ctx::get_view_box_size(self.drawing_ctx);

                rect.width = width
                    .map(|w| w.normalize(values, self.drawing_ctx))
                    .unwrap_or(vbox_width);
                rect.height = height
                    .map(|h| h.normalize(values, self.drawing_ctx))
                    .unwrap_or(vbox_height);
            }

            if filter.primitiveunits.get() == CoordUnits::ObjectBoundingBox {
                drawing_ctx::pop_view_box(self.drawing_ctx);
            }

            let other_bbox = BoundingBox::new(&self.paffine).with_rect(Some(rect));
            bbox.clip(&other_bbox);
        }

        let rect = cairo::Rectangle {
            x: 0f64,
            y: 0f64,
            width: f64::from(self.source_surface.get_width()),
            height: f64::from(self.source_surface.get_height()),
        };
        let other_bbox = BoundingBox::new(&cairo::Matrix::identity()).with_rect(Some(rect));
        bbox.clip(&other_bbox);

        let bbox_rect = bbox.rect.unwrap();
        IRect {
            x0: bbox_rect.x.floor() as i32,
            y0: bbox_rect.y.floor() as i32,
            x1: (bbox_rect.x + bbox_rect.width).ceil() as i32,
            y1: (bbox_rect.y + bbox_rect.height).ceil() as i32,
        }
    }

    /// Retrieves the filter input surface according to the SVG rules.
    pub fn get_input(&self, in_: Option<&Input>) -> Option<FilterOutput> {
        if in_.is_none() {
            // No value => use the last result.
            // As per the SVG spec, if the filter primitive is the first in the chain, return the
            // source graphic.
            return Some(self.last_result().cloned().unwrap_or_else(|| FilterOutput {
                surface: self.source_graphic().clone(),
                bounds: self.compute_bounds(None, None, None, None),
            }));
        }

        match *in_.unwrap() {
            Input::SourceGraphic => Some(FilterOutput {
                surface: self.source_graphic().clone(),
                bounds: self.compute_bounds(None, None, None, None),
            }),
            Input::SourceAlpha => {
                let bounds = self.compute_bounds(None, None, None, None);
                self.source_alpha(bounds)
                    .ok()
                    .map(|surface| FilterOutput { surface, bounds })
            }
            Input::BackgroundImage => self.background_image().ok().map(|surface| FilterOutput {
                surface: surface.clone(),
                bounds: self.compute_bounds(None, None, None, None),
            }),
            Input::BackgroundAlpha => {
                let bounds = self.compute_bounds(None, None, None, None);
                self.background_alpha(bounds)
                    .ok()
                    .map(|surface| FilterOutput { surface, bounds })
            }

            // TODO
            Input::FillPaint => None,
            Input::StrokePaint => None,

            Input::FilterOutput(ref name) => self.filter_output(name).cloned(),
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

    (*ctx).source_surface.get_width()
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_height(ctx: *const RsvgFilterContext) -> i32 {
    assert!(!ctx.is_null());

    (*ctx).source_surface.get_height()
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
            bounds: ctx.compute_bounds(None, None, None, None),
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

    let surface: cairo::Surface = from_glib_none(result.surface);
    assert_eq!(surface.get_type(), cairo::SurfaceType::Image);
    let surface = cairo::ImageSurface::from(surface).unwrap();

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

    ctx.compute_bounds(x, y, width, height)
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
        Some(FilterOutput { surface, bounds }) => {
            // HACK because to_glib_full() is unimplemented!() on ImageSurface.
            let ptr = surface.to_glib_none().0;
            mem::forget(surface);

            RsvgFilterPrimitiveOutput {
                surface: ptr,
                bounds,
            }
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
        const WIDTH: usize = 32;
        const HEIGHT: usize = 64;
        const BOUNDS: IRect = IRect {
            x0: 8,
            x1: 24,
            y0: 16,
            y1: 48,
        };
        const FULL_BOUNDS: IRect = IRect {
            x0: 0,
            x1: WIDTH as i32,
            y0: 0,
            y1: HEIGHT as i32,
        };

        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, WIDTH as i32, HEIGHT as i32)
                .unwrap();

        // Fill the surface with some data.
        {
            let mut data = surface.get_data().unwrap();

            let mut counter = 0u16;
            for x in data.iter_mut() {
                *x = counter as u8;
                counter = (counter + 1) % 256;
            }
        }

        let alpha = extract_alpha(&surface, BOUNDS).unwrap();

        let data = ImageSurfaceDataShared::new(&surface).unwrap();
        let data_alpha = ImageSurfaceDataShared::new(&alpha).unwrap();

        for (x, y, p, pa) in
            Pixels::new(data, FULL_BOUNDS).map(|(x, y, p)| (x, y, p, data_alpha.get_pixel(x, y)))
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
