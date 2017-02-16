use drawing_ctx::RsvgDrawingCtx;
use handle::*;
use node::*;
use property_bag::RsvgPropertyBag;
use state::RsvgState;

use std::rc::*;

type CNodeSetAtts = unsafe extern "C" fn (node: *const RsvgNode, node_impl: *const RsvgCNodeImpl, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag);
type CNodeDraw = unsafe extern "C" fn (node: *const RsvgNode, node_impl: *const RsvgCNodeImpl, draw_ctx: *const RsvgDrawingCtx, dominate: i32);
type CNodeFree = unsafe extern "C" fn (node_impl: *const RsvgCNodeImpl);

struct CNode {
    c_node_impl: *const RsvgCNodeImpl,

    set_atts_fn: CNodeSetAtts,
    draw_fn:     CNodeDraw,
    free_fn:     CNodeFree,
}

impl NodeTrait for CNode {
    fn set_atts (&self, node: &RsvgNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        unsafe { (self.set_atts_fn) (node as *const RsvgNode, self.c_node_impl, handle, pbag); }
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        unsafe { (self.draw_fn) (node as *const RsvgNode, self.c_node_impl, draw_ctx, dominate); }
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        self.c_node_impl
    }
}

impl Drop for CNode {
    fn drop (&mut self) {
        unsafe { (self.free_fn) (self.c_node_impl); }
    }
}

#[no_mangle]
pub extern fn rsvg_rust_cnode_new (node_type:   NodeType,
                                   raw_parent:  *const RsvgNode,
                                   state:       *mut RsvgState,
                                   c_node_impl: *const RsvgCNodeImpl,
                                   set_atts_fn: CNodeSetAtts,
                                   draw_fn:     CNodeDraw,
                                   free_fn:     CNodeFree) -> *const RsvgNode {
    assert! (!state.is_null ());
    assert! (!c_node_impl.is_null ());

    let parent: Option<Weak<Node>> =
        if raw_parent.is_null () {
            None
        } else {
            let p: &RsvgNode = unsafe { & *raw_parent };
            Some (Rc::downgrade (&p.clone ()))
        };

    let cnode = CNode {
        c_node_impl: c_node_impl,
        set_atts_fn: set_atts_fn,
        draw_fn:     draw_fn,
        free_fn:     free_fn
    };

    box_node (Rc::new (Node::new (node_type,
                                  parent,
                                  state,
                                  Box::new (cnode))))
}

#[no_mangle]
pub extern fn rsvg_rust_cnode_get_impl (raw_node: *const RsvgNode) -> *const RsvgCNodeImpl {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.get_c_impl ()
}
