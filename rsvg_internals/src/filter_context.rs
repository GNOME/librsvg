use std::collections::HashMap;
use std::ffi::CStr;

use cairo::prelude::SurfaceExt;
use cairo::{self, MatrixTrait};
use cairo_sys::cairo_surface_t;
use glib::translate::{from_glib_none, ToGlibPtr};
use glib_sys::*;

use bbox::BoundingBox;
use coord_units::CoordUnits;
use drawing_ctx::{self, RsvgDrawingCtx};
use filters::{IRect, RsvgFilterPrimitive};
use length::RsvgLength;

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/rsvg-filter.g:_RsvgFilter
#[repr(C)]
pub struct RsvgFilter {
    pub x: RsvgLength,
    pub y: RsvgLength,
    pub width: RsvgLength,
    pub height: RsvgLength,
    pub filterunits: CoordUnits,
    pub primitiveunits: CoordUnits,
}

pub type RsvgFilterContext = FilterContext;

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterPrimitiveOutput
#[repr(C)]
pub struct RsvgFilterPrimitiveOutput {
    surface: *mut cairo_surface_t,
    bounds: IRect,
}

/// A filter primitive result.
#[derive(Debug, Clone)]
pub struct FilterResult {
    /// The surface after the filter primitive was applied.
    pub surface: cairo::ImageSurface,

    /// The filter primitive subregion.
    pub bounds: IRect,
}

/// The filter rendering context.
#[derive(Debug)]
pub struct FilterContext {
    /// The source graphic surface.
    source_surface: cairo::ImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<FilterResult>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<String, FilterResult>,

    affine: cairo::Matrix,
    paffine: cairo::Matrix,
    filter: *mut RsvgFilter,
    drawing_ctx: *mut RsvgDrawingCtx,
    channelmap: [i32; 4],
}

impl FilterContext {
    /// Creates a new `FilterContext`.
    pub fn new(
        filter: *mut RsvgFilter,
        source_surface: cairo::ImageSurface,
        ctx: *mut RsvgDrawingCtx,
        channelmap: [i32; 4],
    ) -> Self {
        assert!(!filter.is_null());

        let state = drawing_ctx::get_current_state(ctx).unwrap();
        let bbox = drawing_ctx::get_bbox(ctx);
        let bbox_rect = bbox.rect.unwrap();

        let affine = match unsafe { (*filter).filterunits } {
            CoordUnits::UserSpaceOnUse => state.affine,
            CoordUnits::ObjectBoundingBox => {
                let affine = cairo::Matrix::new(
                    bbox_rect.width,
                    0f64,
                    0f64,
                    bbox_rect.height,
                    bbox_rect.x,
                    bbox_rect.y,
                );
                cairo::Matrix::multiply(&affine, &state.affine)
            }
        };

        let paffine = match unsafe { (*filter).primitiveunits } {
            CoordUnits::UserSpaceOnUse => state.affine,
            CoordUnits::ObjectBoundingBox => {
                let affine = cairo::Matrix::new(
                    bbox_rect.width,
                    0f64,
                    0f64,
                    bbox_rect.height,
                    bbox_rect.x,
                    bbox_rect.y,
                );
                cairo::Matrix::multiply(&affine, &state.affine)
            }
        };

        let mut rv = Self {
            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
            affine,
            paffine,
            filter,
            drawing_ctx: ctx,
            channelmap,
        };

        let last_result = FilterResult {
            surface: rv.source_surface.clone(),
            bounds: rv.compute_bounds(None, None, None, None),
        };

        rv.last_result = Some(last_result);
        rv
    }

    /// Returns the surface corresponding to the last filter primitive's result.
    #[inline]
    pub fn last_result(&self) -> Option<&FilterResult> {
        self.last_result.as_ref()
    }

    /// Returns the surface corresponding to the source graphic.
    #[inline]
    pub fn source_graphic(&self) -> &cairo::ImageSurface {
        &self.source_surface
    }

    /// Returns the surface corresponding to the background image snapshot.
    #[inline]
    pub fn background_image(&self) -> &cairo::ImageSurface {
        unimplemented!()
    }

    /// Returns the surface corresponding to the result of the given filter primitive.
    #[inline]
    pub fn filter_result(&self, name: &str) -> Option<&FilterResult> {
        self.previous_results.get(name)
    }

    /// Converts this `FilterContext` into the surface corresponding to the result of the filter
    /// chain.
    #[inline]
    pub fn into_result(self) -> cairo::ImageSurface {
        self.last_result
            .map(|FilterResult { surface, .. }| surface)
            .unwrap_or(self.source_surface)
    }

    /// Stores a filter primitive result into the context.
    #[inline]
    pub fn store_result(&mut self, name: Option<String>, result: FilterResult) {
        if let Some(name) = name {
            self.previous_results.insert(name, result.clone());
        }

        self.last_result = Some(result);
    }

    /// Returns the drawing context for this filter context.
    #[inline]
    pub fn drawing_context(&self) -> *const RsvgDrawingCtx {
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
        let mut bbox = BoundingBox::new(&cairo::Matrix::identity());

        if unsafe { (*self.filter).filterunits } == CoordUnits::ObjectBoundingBox {
            drawing_ctx::push_view_box(self.drawing_ctx, 1f64, 1f64);
        }

        let rect = cairo::Rectangle {
            x: unsafe { (*self.filter).x.normalize(self.drawing_ctx) },
            y: unsafe { (*self.filter).y.normalize(self.drawing_ctx) },
            width: unsafe { (*self.filter).width.normalize(self.drawing_ctx) },
            height: unsafe { (*self.filter).height.normalize(self.drawing_ctx) },
        };

        if unsafe { (*self.filter).filterunits } == CoordUnits::ObjectBoundingBox {
            drawing_ctx::pop_view_box(self.drawing_ctx);
        }

        let other_bbox = BoundingBox::new(&self.affine).with_rect(Some(rect));
        bbox.insert(&other_bbox);

        if x.is_some() || y.is_some() || width.is_some() || height.is_some() {
            if unsafe { (*self.filter).primitiveunits } == CoordUnits::ObjectBoundingBox {
                drawing_ctx::push_view_box(self.drawing_ctx, 1f64, 1f64);
            }

            let mut rect = cairo::Rectangle {
                x: x.map(|x| x.normalize(self.drawing_ctx)).unwrap_or(0f64),
                y: y.map(|y| y.normalize(self.drawing_ctx)).unwrap_or(0f64),
                ..rect
            };

            if width.is_some() || height.is_some() {
                let (vbox_width, vbox_height) = drawing_ctx::get_view_box_size(self.drawing_ctx);

                rect.width = width
                    .map(|w| w.normalize(self.drawing_ctx))
                    .unwrap_or(vbox_width);
                rect.height = height
                    .map(|h| h.normalize(self.drawing_ctx))
                    .unwrap_or(vbox_height);
            }

            if unsafe { (*self.filter).primitiveunits } == CoordUnits::ObjectBoundingBox {
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
            x0: bbox_rect.x as i32,
            y0: bbox_rect.y as i32,
            x1: (bbox_rect.x + bbox_rect.width) as i32,
            y1: (bbox_rect.y + bbox_rect.height) as i32,
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
pub unsafe extern "C" fn rsvg_filter_context_get_filter(
    ctx: *const RsvgFilterContext,
) -> *const RsvgFilter {
    assert!(!ctx.is_null());

    (*ctx).filter
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

    (*ctx).background_image().to_glib_none().0
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_context_get_lastresult(
    ctx: *mut RsvgFilterContext,
) -> RsvgFilterPrimitiveOutput {
    assert!(!ctx.is_null());

    match (*ctx).last_result {
        Some(FilterResult {
            ref surface,
            ref bounds,
        }) => RsvgFilterPrimitiveOutput {
            surface: surface.to_glib_none().0,
            bounds: *bounds,
        },
        None => RsvgFilterPrimitiveOutput {
            surface: (*ctx).source_surface.to_glib_none().0,
            bounds: (*ctx).compute_bounds(None, None, None, None),
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

    if let Some(&FilterResult {
        ref surface,
        ref bounds,
    }) = (*ctx).filter_result(&CStr::from_ptr((*name).str).to_string_lossy())
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
        surface,
        bounds: result.bounds,
    };

    (*ctx).store_result(Some(name), result);
}
