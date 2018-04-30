//! Internal FFI and marshalling things.

use std::default::Default;
use std::ptr;

use glib_sys::{gboolean, GString};

use length::RsvgLength;
use node::RsvgNode;
use filter_context::RsvgFilterContext;

use super::Filter;

// Keep this in sync with
// ../../librsvg/librsvg/filters/common.h:_RsvgFilterPrimitive
/// Required by the C code until all filters are ported to Rust.
#[repr(C)]
pub(super) struct RsvgFilterPrimitive {
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

impl RsvgFilterPrimitive {
    /// Creates a new `RsvgFilterPrimitive` with the proper render callback.
    #[inline]
    pub(super) fn new<T: Filter>() -> RsvgFilterPrimitive {
        RsvgFilterPrimitive {
            x: Default::default(),
            y: Default::default(),
            width: Default::default(),
            height: Default::default(),
            x_specified: Default::default(),
            y_specified: Default::default(),
            width_specified: Default::default(),
            height_specified: Default::default(),
            in_: ptr::null_mut(),
            result: ptr::null_mut(),

            render: Some(render_callback::<T>),
        }
    }
}

/// Calls `Filter::render()` for this filter primitive.
///
/// # Safety
/// `raw_node` and `ctx` must be valid pointers.
unsafe extern "C" fn render_callback<T: Filter>(
    raw_node: *mut RsvgNode,
    _primitive: *mut RsvgFilterPrimitive,
    ctx: *mut RsvgFilterContext,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = &*raw_node;

    node.with_impl(move |filter: &T| filter.render(ctx));
}
