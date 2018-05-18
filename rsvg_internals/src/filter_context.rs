use std::collections::HashMap;
use std::{mem, ptr};

use cairo;
use cairo_sys::cairo_surface_t;
use glib::translate::{FromGlibPtrBorrow, FromGlibPtrFull, ToGlibPtr};
use glib_sys::*;
use libc::c_char;

use bbox::RsvgBbox;
use drawing_ctx::{self, RsvgDrawingCtx};
use filters::{IRect, RsvgFilterPrimitive};
use state::RsvgState;
use util::utf8_cstr;

// Required by the C code until all filters are ported to Rust.
pub enum RsvgFilter {}

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterPrimitiveOutput
#[repr(C)]
struct RsvgFilterPrimitiveOutput {
    surface: *mut cairo_surface_t,
    bounds: IRect,
}

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterContext
#[repr(C)]
pub struct RsvgFilterContext {
    width: i32,
    height: i32,
    filter: *mut RsvgFilter,
    results: *mut GHashTable,
    source_surface: *mut cairo_surface_t,
    bg_surface: *mut cairo_surface_t,
    lastresult: RsvgFilterPrimitiveOutput,
    affine: cairo::Matrix,
    paffine: cairo::Matrix,
    channelmap: [i32; 4],
    ctx: *mut RsvgDrawingCtx,
}

/// The filter rendering context.
pub struct FilterContext {
    // TODO: remove when all filters are ported to Rust.
    /// The C struct passed to C filters.
    c_struct: RsvgFilterContext,

    /// The source graphic surface.
    source_surface: cairo::ImageSurface,
    /// Output of the last filter primitive.
    last_result: Option<cairo::ImageSurface>,
    /// Surfaces of the previous filter primitives by name.
    previous_results: HashMap<String, cairo::ImageSurface>,
}

impl FilterContext {
    /// Creates a new `FilterContext`.
    #[inline]
    pub fn new(
        filter: *mut RsvgFilter,
        source_surface: cairo::ImageSurface,
        ctx: *mut RsvgDrawingCtx,
        channelmap: [i32; 4],
    ) -> Self {
        extern "C" {
            fn rsvg_filter_fix_coordinate_system(
                ctx: *mut RsvgFilterContext,
                state: *mut RsvgState,
                bbox: *mut RsvgBbox, // Actually *const.
            );

            fn rsvg_filter_free_pair(value: gpointer);

            fn rsvg_filter_primitive_get_bounds(
                primitive: *mut RsvgFilterPrimitive,
                ctx: *mut RsvgFilterContext, // Actually *const.
            ) -> IRect;
        }

        let mut rv = Self {
            c_struct: RsvgFilterContext {
                filter,
                results: unsafe {
                    g_hash_table_new_full(
                        Some(g_str_hash),
                        Some(g_str_equal),
                        Some(g_free),
                        Some(rsvg_filter_free_pair),
                    )
                },
                source_surface: source_surface.to_glib_full(),
                bg_surface: ptr::null_mut(),
                lastresult: RsvgFilterPrimitiveOutput {
                    surface: source_surface.to_glib_full(),

                    // Initialized by rsvg_filter_primitive_get_bounds().
                    bounds: unsafe { mem::uninitialized() },
                },

                channelmap,
                ctx,

                // This stuff is initialized by rsvg_filter_fix_coordinate_system().
                width: unsafe { mem::uninitialized() },
                height: unsafe { mem::uninitialized() },
                affine: unsafe { mem::uninitialized() },
                paffine: unsafe { mem::uninitialized() },
            },

            source_surface,
            last_result: None,
            previous_results: HashMap::new(),
        };

        unsafe {
            rsvg_filter_fix_coordinate_system(
                &mut rv.c_struct,
                drawing_ctx::get_current_state_ptr(ctx),
                drawing_ctx::get_bbox(ctx) as *mut RsvgBbox,
            )
        };

        rv.c_struct.lastresult.bounds =
            unsafe { rsvg_filter_primitive_get_bounds(ptr::null_mut(), &mut rv.c_struct) };

        rv
    }

    /// Returns a pointer to the raw C struct.
    #[inline]
    pub fn get_raw(&self) -> *const RsvgFilterContext {
        &self.c_struct
    }

    /// Returns a mutable pointer to the raw C struct.
    #[inline]
    pub fn get_raw_mut(&mut self) -> *mut RsvgFilterContext {
        &mut self.c_struct
    }

    /// Refreshes the Rust fields from the C struct fields that could get modified from within the
    /// filter code.
    pub fn refresh_from_c(&mut self) {
        self.last_result = Some(unsafe {
            cairo::ImageSurface::from_glib_borrow(self.c_struct.lastresult.surface)
        });

        // Update the previous results map.
        unsafe {
            let mut iter = mem::uninitialized();
            g_hash_table_iter_init(&mut iter, self.c_struct.results);

            let mut key: *const c_char = mem::uninitialized();
            let mut value: *const RsvgFilterPrimitiveOutput = mem::uninitialized();
            while g_hash_table_iter_next(
                &mut iter,
                &mut key as *mut _ as *mut gpointer,
                &mut value as *mut _ as *mut gpointer,
            ) != 0
            {
                if !self.previous_results.contains_key(utf8_cstr(key)) {
                    self.previous_results.insert(
                        utf8_cstr(key).to_owned(),
                        cairo::ImageSurface::from_glib_borrow((*value).surface),
                    );
                }
            }
        }
    }

    /// Returns the surface corresponding to the last filter primitive's result.
    #[inline]
    pub fn last_result(&self) -> Option<cairo::ImageSurface> {
        self.last_result.clone()
    }

    /// Returns the surface corresponding to the source graphic.
    #[inline]
    pub fn source_graphic(&self) -> cairo::ImageSurface {
        self.source_surface.clone()
    }

    /// Returns the surface corresponding to the background image snapshot.
    #[inline]
    pub fn background_image(&self) -> cairo::ImageSurface {
        unimplemented!()
    }

    /// Returns the surface corresponding to the result of the given filter primitive.
    #[inline]
    pub fn filter_result(&self, name: &str) -> Option<cairo::ImageSurface> {
        self.previous_results.get(name).cloned()
    }

    /// Converts this `FilterContext` into the surface corresponding to the result of the filter
    /// chain.
    #[inline]
    pub fn into_result(self) -> cairo::ImageSurface {
        self.last_result
            .clone()
            .unwrap_or_else(|| self.source_surface.clone())
    }
}

impl Drop for FilterContext {
    fn drop(&mut self) {
        unsafe {
            drop(cairo::Surface::from_glib_full(self.c_struct.source_surface));
            drop(cairo::Surface::from_glib_full(
                self.c_struct.lastresult.surface,
            ));
            g_hash_table_destroy(self.c_struct.results);
            if !self.c_struct.bg_surface.is_null() {
                drop(cairo::Surface::from_glib_full(self.c_struct.bg_surface));
            }
        }
    }
}
