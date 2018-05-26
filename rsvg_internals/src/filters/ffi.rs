//! Internal FFI and marshalling things.

use std::mem;

use cairo;
use cairo::prelude::SurfaceExt;
use cairo_sys::cairo_surface_t;
use glib::translate::{from_glib_borrow, ToGlibPtr};
use glib_sys::*;
use libc::c_char;

use drawing_ctx::RsvgDrawingCtx;
use filter_context::{FilterContext, RsvgFilter, RsvgFilterContext};
use length::RsvgLength;
use node::{NodeType, RsvgCNodeImpl, RsvgNode};
use state::{ComputedValues, RsvgComputedValues};

use super::Filter;

// Required by the C code until all filters are ported to Rust.
// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterPrimitive
#[repr(C)]
pub struct RsvgFilterPrimitive {
    pub x: RsvgLength,
    pub y: RsvgLength,
    pub width: RsvgLength,
    pub height: RsvgLength,
    pub x_specified: gboolean,
    pub y_specified: gboolean,
    pub width_specified: gboolean,
    pub height_specified: gboolean,
    in_: *mut GString,
    result: *mut GString,

    render: Option<
        unsafe extern "C" fn(
            *mut RsvgNode,
            RsvgComputedValues,
            *mut RsvgFilterPrimitive,
            *mut RsvgFilterContext,
        ),
    >,
}

/// The type of the render function below.
pub(super) type RenderFunctionType = fn(&RsvgNode, &mut FilterContext);

/// Downcasts the given `node` to the type `T` and calls `Filter::render()` on it.
pub(super) fn render<T: Filter>(node: &RsvgNode, ctx: &mut FilterContext) {
    node.with_impl(|filter: &T| filter.render(node, ctx));
}

/// Creates a new surface applied the filter. This function will create a context for itself, set up
/// the coordinate systems execute all its little primitives and then clean up its own mess.
#[no_mangle]
pub unsafe extern "C" fn rsvg_filter_render(
    filter_node: *mut RsvgNode,
    source: *mut cairo_surface_t,
    context: *mut RsvgDrawingCtx,
    channelmap: *const c_char,
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

    let values = &filter_node.get_computed_values() as &ComputedValues;

    let mut channelmap_arr = [0; 4];
    for i in 0..4 {
        channelmap_arr[i] = i32::from(*channelmap.offset(i as isize) - '0' as i8);
    }

    let mut filter_ctx = FilterContext::new(
        filter_node.get_c_impl() as *mut RsvgFilter,
        filter_node,
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
                let render =
                    *(&c.get_c_impl() as *const *const RsvgCNodeImpl as *const RenderFunctionType);
                render(&c, &mut filter_ctx);
            }
            _ => {
                let filter = &mut *(c.get_c_impl() as *mut RsvgFilterPrimitive);
                (filter.render.unwrap())(
                    &mut c,
                    values as RsvgComputedValues,
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
