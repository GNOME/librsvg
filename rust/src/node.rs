extern crate libc;

use std::rc::Rc;
use std::rc::Weak;
use std::cell::RefCell;
use std::ptr;

use drawing_ctx::RsvgDrawingCtx;
use drawing_ctx;

use handle::RsvgHandle;

use property_bag::RsvgPropertyBag;

use state::RsvgState;

/* A *const RsvgNode is just a pointer for the C code's benefit: it
 * points to an  Rc<Node>, which is our refcounted Rust representation
 * of nodes.
 */
pub type RsvgNode = Rc<Node>;

/* A *const RsvgCNodeImpl is just an opaque pointer to the C code's
 * struct for a particular node type.
 */
pub enum RsvgCNodeImpl {}

pub trait NodeTrait {
    fn set_atts (&self, node: &RsvgNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag);
    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32);
    fn get_c_impl (&self) -> *const RsvgCNodeImpl;
}

pub struct Node {
    node_type: NodeType,
    parent:    Option<Weak<Node>>,       // optional; weak ref to parent
    children:  RefCell<Vec<Rc<Node>>>,   // strong references to children
    state:     *mut RsvgState,
    node_impl: Box<NodeTrait>
}

/* Keep this in sync with rsvg-private.h:RsvgNodeType */
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NodeType {
    Invalid = 0,

    Chars,
    Circle,
    ClipPath,
    ComponentTransferFunction,
    Defs,
    Ellipse,
    Filter,
    Group,
    Image,
    LightSource,
    Line,
    LinearGradient,
    Marker,
    Mask,
    Path,
    Pattern,
    Polygon,
    Polyline,
    RadialGradient,
    Rect,
    Stop,
    Svg,
    Switch,
    Symbol,
    Text,
    TRef,
    TSpan,
    Use,

    /* Filter primitives */
    FilterPrimitiveFirst,              /* just a marker; not a valid type */
    FilterPrimitiveBlend,
    FilterPrimitiveColorMatrix,
    FilterPrimitiveComponentTransfer,
    FilterPrimitiveComposite,
    FilterPrimitiveConvolveMatrix,
    FilterPrimitiveDiffuseLighting,
    FilterPrimitiveDisplacementMap,
    FilterPrimitiveErode,
    FilterPrimitiveFlood,
    FilterPrimitiveGaussianBlur,
    FilterPrimitiveImage,
    FilterPrimitiveMerge,
    FilterPrimitiveMergeNode,
    FilterPrimitiveOffset,
    FilterPrimitiveSpecularLighting,
    FilterPrimitiveTile,
    FilterPrimitiveTurbulence,
    FilterPrimitiveLast                /* just a marker; not a valid type */
}

impl Node {
    pub fn new (node_type: NodeType,
                parent:    Option<Weak<Node>>,
                state:     *mut RsvgState,
                node_impl: Box<NodeTrait>) -> Node {
        Node {
            node_type: node_type,
            parent:    parent,
            children:  RefCell::new (Vec::new ()),
            state:     state,
            node_impl: node_impl
        }
    }

    pub fn get_type (&self) -> NodeType {
        self.node_type
    }

    pub fn get_state (&self) -> *mut RsvgState {
        self.state
    }

    pub fn add_child (&self, child: &Rc<Node>) {
        self.children.borrow_mut ().push (child.clone ());
    }

    pub fn set_atts (&self, node: &RsvgNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        self.node_impl.set_atts (node, handle, pbag);
    }

    pub fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        self.node_impl.draw (node, draw_ctx, dominate);
    }

    pub fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        self.node_impl.get_c_impl ()
    }
}

extern "C" {
    fn rsvg_state_free (state: *mut RsvgState);
}

impl Drop for Node {
    fn drop (&mut self) {
//        unsafe { rsvg_state_free (self.state); }
    }
}

pub fn node_ptr_to_weak (raw_parent: *const RsvgNode) -> Option<Weak<Node>> {
    if raw_parent.is_null () {
        None
    } else {
        let p: &RsvgNode = unsafe { & *raw_parent };
        Some (Rc::downgrade (&p.clone ()))
    }
}

pub fn boxed_node_new (node_type:  NodeType,
                       raw_parent: *const RsvgNode,
                       node_impl: Box<NodeTrait>) -> *mut RsvgNode {
    box_node (Rc::new (Node::new (node_type,
                                  node_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  node_impl)))
}

#[no_mangle]
pub extern fn rsvg_node_get_type (raw_node: *const RsvgNode) -> NodeType {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.get_type ()
}

pub fn box_node (node: RsvgNode) -> *mut RsvgNode {
    Box::into_raw (Box::new (node))
}

#[no_mangle]
pub extern fn rsvg_node_get_parent (raw_node: *const RsvgNode) -> *const RsvgNode {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    match node.parent {
        None => { ptr::null () }

        Some (ref weak_node) => {
            let strong_node = weak_node.upgrade ().unwrap ();
            box_node (strong_node)
        }
    }
}

#[no_mangle]
pub extern fn rsvg_node_ref (raw_node: *mut RsvgNode) -> *mut RsvgNode {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    box_node (node.clone ())
}

#[no_mangle]
pub extern fn rsvg_node_unref (raw_node: *mut RsvgNode) -> *mut RsvgNode {
    if !raw_node.is_null () {
        let _ = unsafe { Box::from_raw (raw_node) };
    }

    ptr::null_mut () // so the caller can do "node = rsvg_node_unref (node);" and lose access to the node
}

// See https://github.com/rust-lang/rust/issues/36497 - this is what
// added Rc::ptr_eq(), but we don't want to depend on unstable Rust
// just yet.

fn rc_node_ptr_eq<T: ?Sized> (this: &Rc<T>, other: &Rc<T>) -> bool {
    let this_ptr: *const T = &**this;
    let other_ptr: *const T = &**other;
    this_ptr == other_ptr
}

#[no_mangle]
pub extern fn rsvg_node_is_same (raw_node1: *const RsvgNode, raw_node2: *const RsvgNode) -> bool {
    if raw_node1.is_null () && raw_node2.is_null () {
        true
    } else if !raw_node1.is_null () && !raw_node2.is_null () {
        let node1: &RsvgNode = unsafe { & *raw_node1 };
        let node2: &RsvgNode = unsafe { & *raw_node2 };

        rc_node_ptr_eq (node1, node2)
    } else {
        false
    }
}

#[no_mangle]
pub extern fn rsvg_node_get_state (raw_node: *const RsvgNode) -> *mut RsvgState {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.get_state ()
}

#[no_mangle]
pub extern fn rsvg_node_add_child (raw_node: *mut RsvgNode, raw_child: *const RsvgNode) {
    assert! (!raw_node.is_null ());
    assert! (!raw_child.is_null ());
    let node: &mut RsvgNode = unsafe { &mut *raw_node };
    let child: &RsvgNode = unsafe { & *raw_child };

    node.add_child (child);
}

#[no_mangle]
pub extern fn rsvg_node_set_atts (raw_node: *mut RsvgNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.set_atts (node, handle, pbag);
}

#[no_mangle]
pub extern fn rsvg_node_draw (raw_node: *const RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.draw (node, draw_ctx, dominate);
}

type NodeForeachChild = unsafe extern "C" fn (node: *const RsvgNode, data: *const libc::c_void) -> bool;

#[no_mangle]
pub extern fn rsvg_node_foreach_child (raw_node: *const RsvgNode, func: NodeForeachChild, data: *const libc::c_void)
{
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    for child in &*node.children.borrow () {
        let boxed_child = box_node (child.clone ());

        let next = unsafe { func (boxed_child, data) };

        rsvg_node_unref (boxed_child);

        if !next {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use drawing_ctx::RsvgDrawingCtx;
    use handle::RsvgHandle;
    use property_bag::RsvgPropertyBag;
    use state::RsvgState;
    use super::*;
    use std::ptr;

    struct TestNodeImpl {}

    impl NodeTrait for TestNodeImpl {
        fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) {
        }

        fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        }

        fn get_c_impl (&self) -> *const RsvgCNodeImpl {
            return ptr::null ();
        }
    }

    #[test]
    fn node_refs_and_unrefs () {
        let node = Rc::new (Node::new (NodeType::Path,
                                       None,
                                       ptr::null_mut (),
                                       Box::new (TestNodeImpl {})));

        let ref1 = box_node (node);

        let new_node: &mut RsvgNode = unsafe { &mut *ref1 };
        let weak = Rc::downgrade (new_node);

        let ref2 = rsvg_node_ref (new_node);
        assert! (weak.upgrade ().is_some ());

        rsvg_node_unref (ref2);
        assert! (weak.upgrade ().is_some ());

        rsvg_node_unref (ref1);
        assert! (weak.upgrade ().is_none ());
    }

    #[test]
    fn reffed_node_is_same_as_original_node () {
        let node = Rc::new (Node::new (NodeType::Path,
                                       None,
                                       ptr::null_mut (),
                                       Box::new (TestNodeImpl {})));

        let ref1 = box_node (node);

        let ref2 = rsvg_node_ref (ref1);

        assert! (rsvg_node_is_same (ref1, ref2));

        rsvg_node_unref (ref1);
        rsvg_node_unref (ref2);
    }

    #[test]
    fn different_nodes_have_different_pointers () {
        let node1 = Rc::new (Node::new (NodeType::Path,
                                       None,
                                       ptr::null_mut (),
                                       Box::new (TestNodeImpl {})));

        let ref1 = box_node (node1);

        let node2 = Rc::new (Node::new (NodeType::Path,
                                       None,
                                       ptr::null_mut (),
                                       Box::new (TestNodeImpl {})));

        let ref2 = box_node (node2);

        assert! (!rsvg_node_is_same (ref1, ref2));

        rsvg_node_unref (ref1);
        rsvg_node_unref (ref2);
    }
}
