use std::collections::HashMap;
use std::ffi::CStr;
use std::{mem, ptr, slice};

use cairo::prelude::SurfaceExt;
use cairo::{self, MatrixTrait};
use cairo_sys::cairo_surface_t;
use glib::translate::{from_glib_none, ToGlibPtr};
use glib_sys::*;
use libc::c_char;

use bbox::RsvgBbox;
use coord_units::CoordUnits;
use drawing_ctx::{self, RsvgDrawingCtx};
use filters::{IRect, RsvgFilterPrimitive};
use length::RsvgLength;
use state::RsvgState;
use util::utf8_cstr;

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
    surface: cairo::ImageSurface,

    /// The filter primitive subregion.
    bounds: IRect,
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
        let bbox = unsafe { &*drawing_ctx::get_bbox(ctx) };

        let affine = match unsafe { (*filter).filterunits } {
            CoordUnits::UserSpaceOnUse => state.affine,
            CoordUnits::ObjectBoundingBox => {
                let affine = cairo::Matrix::new(
                    bbox.rect.width,
                    0f64,
                    0f64,
                    bbox.rect.height,
                    bbox.rect.x,
                    bbox.rect.y,
                );
                cairo::Matrix::multiply(&affine, &state.affine)
            }
        };

        let paffine = match unsafe { (*filter).primitiveunits } {
            CoordUnits::UserSpaceOnUse => state.affine,
            CoordUnits::ObjectBoundingBox => {
                let affine = cairo::Matrix::new(
                    bbox.rect.width,
                    0f64,
                    0f64,
                    bbox.rect.height,
                    bbox.rect.x,
                    bbox.rect.y,
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
            bounds: {
                extern "C" {
                    fn rsvg_filter_primitive_get_bounds(
                        primitive: *mut RsvgFilterPrimitive,
                        ctx: *const RsvgFilterContext,
                    ) -> IRect;
                }

                unsafe { rsvg_filter_primitive_get_bounds(ptr::null_mut(), &rv) }
            }
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
    pub fn store_result(&mut self, name: String, result: FilterResult) {
        if !name.is_empty() {
            self.previous_results.insert(name, result.clone());
        }

        self.last_result = Some(result);
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
            bounds: {
                extern "C" {
                    fn rsvg_filter_primitive_get_bounds(
                        primitive: *mut RsvgFilterPrimitive,
                        ctx: *const RsvgFilterContext,
                    ) -> IRect;
                }

                rsvg_filter_primitive_get_bounds(ptr::null_mut(), ctx)
            },
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

    (*ctx).store_result(name, result);
}
