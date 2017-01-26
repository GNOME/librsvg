extern crate libc;

use std::rc::Rc;
use std::rc::Weak;
use std::cell::RefCell;
use std::ptr;

use drawing_ctx::RsvgDrawingCtx;

use handle::RsvgHandle;

use property_bag::RsvgPropertyBag;

use state::RsvgState;

/* A const *RsvgNode is just a pointer for the C code's benefit: it
 * points to a RsvgRcNode, which is our refcounted Rust representation
 * of nodes.
 */
pub enum RsvgNode {}

/* This is just to take a pointer to an Rc<RefCell<Node>> */
pub type RsvgRcNode = Rc<RefCell<Node>>;

pub trait NodeTrait {
    fn set_atts (&self, node: &RsvgRcNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag);
    fn draw (&self, node: &RsvgRcNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32);
}

pub struct Node {
    node_type: NodeType,
    parent:    Option<Weak<RefCell<Node>>>, // optional; weak ref to parent-made-mutable
    children:  Vec<Rc<RefCell<Node>>>,   // strong references to children-made-mutable through RefCell
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
                parent:    Option<Weak<RefCell<Node>>>,
                state:     *mut RsvgState,
                node_impl: Box<NodeTrait>) -> Node {
        Node {
            node_type: node_type,
            parent:    parent,
            children:  Vec::new (),
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

    pub fn add_child (&mut self, child: &Rc<RefCell<Node>>) {
        self.children.push (child.clone ());
    }
}

#[no_mangle]
pub extern fn rsvg_node_get_type (raw_node: *const RsvgRcNode) -> NodeType {
    assert! (!raw_node.is_null ());
    let node: &RsvgRcNode = unsafe { & *raw_node };

    node.borrow ().get_type ()
}

#[no_mangle]
pub extern fn rsvg_node_get_parent (raw_node: *const RsvgRcNode) -> *const RsvgRcNode {
    assert! (!raw_node.is_null ());
    let rc_node: &RsvgRcNode = unsafe { & *raw_node };

    match rc_node.borrow ().parent {
        None => { ptr::null () }

        Some (ref weak_node) => {
            let strong_node = weak_node.upgrade ().unwrap ();
            &strong_node as *const RsvgRcNode
        }
    }
}

#[no_mangle]
pub extern fn rsvg_node_unref (raw_node: *const RsvgRcNode) {
    assert! (!raw_node.is_null ());
    let rc_node: &RsvgRcNode = unsafe { & *raw_node };

    drop (rc_node);
}

#[no_mangle]
pub extern fn rsvg_node_get_state (raw_node: *const RsvgRcNode) -> *mut RsvgState {
    assert! (!raw_node.is_null ());
    let rc_node: &RsvgRcNode = unsafe { & *raw_node };

    rc_node.borrow ().get_state ()
}

#[no_mangle]
pub extern fn rsvg_node_add_child (raw_node: *mut RsvgRcNode, raw_child: *const RsvgRcNode) {
    assert! (!raw_node.is_null ());
    assert! (!raw_child.is_null ());
    let rc_node: &mut RsvgRcNode = unsafe { &mut *raw_node };
    let rc_child: &RsvgRcNode = unsafe { & *raw_child };

    rc_node.borrow_mut ().add_child (rc_child);
}
