use glib::translate::*;
use glib_sys;

use drawing_ctx::RsvgDrawingCtx;
use handle::*;
use node::*;
use property_bag::PropertyBag;
use state;

use std::rc::*;

type CNodeSetAtts = unsafe extern "C" fn(
    node: *const RsvgNode,
    node_impl: *const RsvgCNodeImpl,
    handle: *const RsvgHandle,
    pbag: *const PropertyBag,
);
type CNodeDraw = unsafe extern "C" fn(
    node: *const RsvgNode,
    node_impl: *const RsvgCNodeImpl,
    draw_ctx: *mut RsvgDrawingCtx,
    dominate: i32,
    clipping: glib_sys::gboolean,
);
type CNodeFree = unsafe extern "C" fn(node_impl: *const RsvgCNodeImpl);

struct CNode {
    c_node_impl: *const RsvgCNodeImpl,

    set_atts_fn: CNodeSetAtts,
    draw_fn: CNodeDraw,
    free_fn: CNodeFree,
}

impl NodeTrait for CNode {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        unsafe {
            (self.set_atts_fn)(
                node as *const RsvgNode,
                self.c_node_impl,
                handle,
                pbag.ffi(),
            );
        }

        Ok(())
    }

    fn draw(&self, node: &RsvgNode, draw_ctx: *mut RsvgDrawingCtx, dominate: i32, clipping: bool) {
        unsafe {
            (self.draw_fn)(
                node as *const RsvgNode,
                self.c_node_impl,
                draw_ctx,
                dominate,
                clipping.to_glib(),
            );
        }
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.c_node_impl
    }
}

impl Drop for CNode {
    fn drop(&mut self) {
        unsafe {
            (self.free_fn)(self.c_node_impl);
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_rust_cnode_new(
    node_type: NodeType,
    raw_parent: *const RsvgNode,
    c_node_impl: *const RsvgCNodeImpl,
    set_atts_fn: CNodeSetAtts,
    draw_fn: CNodeDraw,
    free_fn: CNodeFree,
) -> *const RsvgNode {
    assert!(!c_node_impl.is_null());

    let cnode = CNode {
        c_node_impl,
        set_atts_fn,
        draw_fn,
        free_fn,
    };

    box_node(Rc::new(Node::new(
        node_type,
        node_ptr_to_weak(raw_parent),
        state::new(),
        Box::new(cnode),
    )))
}

#[no_mangle]
pub extern "C" fn rsvg_rust_cnode_get_impl(raw_node: *const RsvgNode) -> *const RsvgCNodeImpl {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    node.get_c_impl()
}
