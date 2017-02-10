extern crate libc;

use std::rc::Rc;
use std::rc::Weak;
use std::cell::RefCell;
use std::ptr;

use drawing_ctx::RsvgDrawingCtx;

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
        unsafe { rsvg_state_free (self.state); }
    }
}

#[no_mangle]
pub extern fn rsvg_node_get_type (raw_node: *const RsvgNode) -> NodeType {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.get_type ()
}

#[no_mangle]
pub extern fn rsvg_node_get_parent (raw_node: *const RsvgNode) -> *const RsvgNode {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    match node.parent {
        None => { ptr::null () }

        Some (ref weak_node) => {
            let strong_node = weak_node.upgrade ().unwrap ();
            Box::into_raw (Box::new (strong_node))
        }
    }
}

#[no_mangle]
pub unsafe extern fn rsvg_node_unref (raw_node: *mut RsvgNode) {
    assert! (!raw_node.is_null ());

    let _ = Box::from_raw (raw_node);
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

    for child in node.children.borrow ().iter () {
        let next = unsafe { func (child as *const RsvgNode, data) };
        if !next {
            break;
        }
    }
}
