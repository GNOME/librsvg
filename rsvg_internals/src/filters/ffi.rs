//! Internal FFI and marshalling things.

use std::default::Default;
use std::{mem, ptr};

use cairo;
use cairo::prelude::SurfaceExt;
use cairo_sys::cairo_surface_t;
use glib::translate::{from_glib_borrow, from_glib_full, ToGlibPtr};
use glib_sys::*;
use libc::c_char;

use bbox::RsvgBbox;
use drawing_ctx::RsvgDrawingCtx;
use filter_context::{FilterContext, RsvgFilter, RsvgFilterContext};
use length::RsvgLength;
use node::{NodeType, RsvgNode};

use super::Filter;

/// Contains a pointer to the `Filter` trait object. Pointer to this struct is passed back to C
/// code in place of `RsvgFilterPrimitive` for Rust filters. The pointer straight to the trait
/// object cannot be passed as it is a fat pointer.
#[derive(Debug, Clone, Copy)]
pub(super) struct FilterTraitObjectContainer {
    filter: *const Filter,
}

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterPrimitive
#[repr(C)]
pub struct RsvgFilterPrimitive {
    x: RsvgLength,
    y: RsvgLength,
    width: RsvgLength,
    height: RsvgLength,
    x_specified: gboolean,
    y_specified: gboolean,
    width_specified: gboolean,
    height_specified: gboolean,
    in_: *mut GString,
    result: *mut GString,

    render: Option<
        unsafe extern "C" fn(*mut RsvgNode, *mut RsvgFilterPrimitive, *mut RsvgFilterContext),
    >,
}

impl FilterTraitObjectContainer {
    /// Returns the struct with the given pointer.
    #[inline]
    pub(super) fn new(filter: &Filter) -> Self {
        Self { filter }
    }

    /// Returns the struct with an uninitialized pointer.
    #[inline]
    pub(super) fn uninitialized() -> Self {
        Self {
            filter: unsafe { mem::uninitialized() },
        }
    }
}

impl RsvgFilterPrimitive {
    /// Creates a new `RsvgFilterPrimitive` with the given properties.
    #[inline]
    pub(super) fn with_props(
        x: Option<RsvgLength>,
        y: Option<RsvgLength>,
        width: Option<RsvgLength>,
        height: Option<RsvgLength>,
    ) -> Self {
        Self {
            x: x.unwrap_or_else(Default::default),
            y: y.unwrap_or_else(Default::default),
            width: width.unwrap_or_else(Default::default),
            height: height.unwrap_or_else(Default::default),
            x_specified: if x.is_some() { 1 } else { 0 },
            y_specified: if y.is_some() { 1 } else { 0 },
            width_specified: if width.is_some() { 1 } else { 0 },
            height_specified: if height.is_some() { 1 } else { 0 },

            in_: ptr::null_mut(),
            result: ptr::null_mut(),
            render: None,
        }
    }
}

/// Creates a new surface applied the filter. This function will create a context for itself, set up
/// the coordinate systems execute all its little primitives and then clean up its own mess.
#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_render(
    filter_node: *mut RsvgNode,
    source: *mut cairo_surface_t,
    context: *mut RsvgDrawingCtx,
    channelmap: *mut c_char,
) -> *mut cairo_surface_t {
    assert!(!filter_node.is_null());
    assert!(!source.is_null());
    assert!(!context.is_null());
    assert!(!channelmap.is_null());

    let source: cairo::Surface = from_glib_borrow(source);
    assert_eq!(source.get_type(), cairo::SurfaceType::Image);
    let source = cairo::ImageSurface::from(source).unwrap();

    let filter_node = &*filter_node;
    assert_eq!(filter_node.get_type(), NodeType::Filter);

    let mut channelmap_arr = [0; 4];
    for i in 0..4 {
        channelmap_arr[i] = i32::from(*channelmap.offset(i as isize) - '0' as i8);
    }

    let mut filter_ctx = FilterContext::new(
        filter_node.get_c_impl() as *mut RsvgFilter,
        source,
        context,
        channelmap_arr,
    );

    filter_node
        .children()
        .filter(|c| {
            c.get_type() > NodeType::FilterPrimitiveFirst
                && c.get_type() < NodeType::FilterPrimitiveLast
        })
        .filter(|c| !c.is_in_error())
        .for_each(|mut c| match c.get_type() {
            NodeType::FilterPrimitiveOffset => {
                let filter = &*(&*(c.get_c_impl() as *const FilterTraitObjectContainer)).filter;
                filter.render(&mut filter_ctx);
            }
            _ => {
                let filter = &mut *(c.get_c_impl() as *mut RsvgFilterPrimitive);
                (filter.render.unwrap())(
                    &mut c,
                    filter,
                    &mut filter_ctx,
                );
            }
        });

    // HACK because to_glib_full() is unimplemented!() on ImageSurface.
    let result = filter_ctx.into_result();
    let ptr = result.to_glib_none().0;
    mem::forget(result);
    ptr
}
