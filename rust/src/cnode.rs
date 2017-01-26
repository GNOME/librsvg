use drawing_ctx::RsvgDrawingCtx;
use handle::*;
use node::*;
use property_bag::RsvgPropertyBag;
use state::RsvgState;
use std::rc::Rc;
use std::rc::Weak;
use std::cell::RefCell;

/* A *const RsvgCNodeImpl is just an opaque pointer to the C code's
 * struct for a particular node type.
 */
pub enum RsvgCNodeImpl {}

type CNodeSetAtts = unsafe extern "C" fn (node: *const RsvgRcNode, node_impl: *const RsvgCNodeImpl, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag);
type CNodeDraw = unsafe extern "C" fn (node: *const RsvgRcNode, node_impl: *const RsvgCNodeImpl, draw_ctx: *const RsvgDrawingCtx, dominate: i32);
type CNodeFree = unsafe extern "C" fn (node_impl: *const RsvgCNodeImpl);

struct CNode {
    c_node_impl: *const RsvgCNodeImpl,

    set_atts_fn: CNodeSetAtts,
    draw_fn:     CNodeDraw,
    free_fn:     CNodeFree,
}

impl NodeTrait for CNode {
    fn set_atts (&self, node: &RsvgRcNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        unsafe { (self.set_atts_fn) (node as *const RsvgRcNode, self.c_node_impl, handle, pbag); }
    }

    fn draw (&self, node: &RsvgRcNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        unsafe { (self.draw_fn) (node as *const RsvgRcNode, self.c_node_impl, draw_ctx, dominate); }
    }
}

impl Drop for CNode {
    fn drop (&mut self) {
        unsafe { (self.free_fn) (self.c_node_impl); }
    }
}

#[no_mangle]
pub extern fn rsvg_rust_cnode_new (node_type:   NodeType,
                                   raw_parent:  *const RsvgRcNode,
                                   state:       *mut RsvgState,
                                   c_node_impl: *const RsvgCNodeImpl,
                                   set_atts_fn: CNodeSetAtts,
                                   draw_fn:     CNodeDraw,
                                   free_fn:     CNodeFree) -> *const RsvgRcNode {
    assert! (!state.is_null ());
    assert! (!c_node_impl.is_null ());

    let parent: Option<Weak<RefCell<Node>>> = unsafe {
        if raw_parent.is_null () {
            None
        } else {
            Some (Rc::downgrade (&*(raw_parent as *const RsvgRcNode)))
        }
    };

    let cnode = CNode {
        c_node_impl: c_node_impl,
        set_atts_fn: set_atts_fn,
        draw_fn:     draw_fn,
        free_fn:     free_fn
    };

    &Rc::new (RefCell::new (Node::new (node_type,
                                        parent,
                                        state,
                                       Box::new (cnode))))
        as *const RsvgRcNode
}
