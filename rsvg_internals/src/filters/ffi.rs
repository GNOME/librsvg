//! Internal FFI and marshalling things.

use cairo;
use glib_sys::*;
use libc::c_char;

use drawing_ctx::RsvgDrawingCtx;
use length::RsvgLength;
use node::{NodeType, RsvgCNodeImpl, RsvgNode};
use state::{ComputedValues, RsvgComputedValues};
use surface_utils::shared_surface::SharedImageSurface;

use super::context::{FilterContext, RsvgFilterContext};
use super::{Filter, FilterError, FilterResult};

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
pub(super) type RenderFunctionType =
    fn(&RsvgNode, &FilterContext) -> Result<FilterResult, FilterError>;

/// Downcasts the given `node` to the type `T` and calls `Filter::render()` on it.
pub(super) fn render<T: Filter>(
    node: &RsvgNode,
    ctx: &FilterContext,
) -> Result<FilterResult, FilterError> {
    node.with_impl(|filter: &T| filter.render(node, ctx))
}

/// Creates a new surface applied the filter. This function will create a context for itself, set up
/// the coordinate systems execute all its little primitives and then clean up its own mess.
pub fn filter_render(
    filter_node: &RsvgNode,
    node_being_filtered: &RsvgNode,
    source: &cairo::ImageSurface,
    context: *mut RsvgDrawingCtx,
    channelmap: *const c_char,
) -> cairo::ImageSurface {
    assert!(!context.is_null());
    assert!(!channelmap.is_null());

    let filter_node = &*filter_node;
    assert_eq!(filter_node.get_type(), NodeType::Filter);
    assert!(!filter_node.is_in_error());

    let mut channelmap_arr = [0; 4];
    unsafe {
        for i in 0..4 {
            channelmap_arr[i] = i32::from(*channelmap.offset(i as isize) - '0' as i8);
        }
    }

    // The source surface has multiple references. We need to copy it to a new surface to have a
    // unique reference to be able to safely access the pixel data.
    let source_surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        source.get_width(),
        source.get_height(),
    ).unwrap();
    {
        let cr = cairo::Context::new(&source_surface);
        cr.set_source_surface(source, 0f64, 0f64);
        cr.paint();
    }
    let source_surface = SharedImageSurface::new(source_surface).unwrap();

    let mut filter_ctx = FilterContext::new(
        filter_node,
        node_being_filtered,
        source_surface,
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
            NodeType::FilterPrimitiveBlend
            | NodeType::FilterPrimitiveComponentTransfer
            | NodeType::FilterPrimitiveComposite
            | NodeType::FilterPrimitiveFlood
            | NodeType::FilterPrimitiveImage
            | NodeType::FilterPrimitiveMerge
            | NodeType::FilterPrimitiveOffset => {
                let render = unsafe {
                    *(&c.get_c_impl() as *const *const RsvgCNodeImpl as *const RenderFunctionType)
                };
                match render(&c, &filter_ctx) {
                    Ok(result) => filter_ctx.store_result(result),
                    Err(_) => { /* Do nothing for now */ }
                }
            }
            _ => {
                let filter = unsafe { &mut *(c.get_c_impl() as *mut RsvgFilterPrimitive) };
                unsafe {
                    (filter.render.unwrap())(
                        &mut c,
                        &c.get_cascaded_values().get() as &ComputedValues as RsvgComputedValues,
                        filter,
                        &mut filter_ctx,
                    );
                }
            }
        });

    filter_ctx.into_output().into_image_surface()
}
