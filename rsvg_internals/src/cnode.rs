use drawing_ctx::RsvgDrawingCtx;
use handle::*;
use node::*;
use property_bag::PropertyBag;
use state::rsvg_state_new;

use std::rc::*;

type CNodeSetAtts = unsafe extern "C" fn(
    node: *const RsvgNode,
    node_impl: *const RsvgCNodeImpl,
    handle: *const RsvgHandle,
    pbag: *const PropertyBag,
);
type CNodeFree = unsafe extern "C" fn(node_impl: *const RsvgCNodeImpl);

struct CNode {
    c_node_impl: *const RsvgCNodeImpl,

    set_atts_fn: CNodeSetAtts,
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

    fn draw(
        &self,
        _node: &RsvgNode,
        _cascaded: &CascadedValues,
        _draw_ctx: *mut RsvgDrawingCtx,
        _dominate: i32,
        _clipping: bool,
    ) {
        // nothing; the only remaining cnodes are filters, and those don't draw() themselves
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
    free_fn: CNodeFree,
) -> *const RsvgNode {
    assert!(!c_node_impl.is_null());

    let cnode = CNode {
        c_node_impl,
        set_atts_fn,
        free_fn,
    };

    box_node(Rc::new(Node::new(
        node_type,
        node_ptr_to_weak(raw_parent),
        rsvg_state_new(),
        Box::new(cnode),
    )))
}

#[no_mangle]
pub extern "C" fn rsvg_rust_cnode_get_impl(raw_node: *const RsvgNode) -> *const RsvgCNodeImpl {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    node.get_c_impl()
}
