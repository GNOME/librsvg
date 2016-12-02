extern crate libc;

use std::rc::Rc;
use std::rc::Weak;
use std::cell::Cell;
use std::cell::RefCell;
use std::ptr;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;

use handle::RsvgHandle;

use property_bag::RsvgPropertyBag;

use state::RsvgState;

pub trait NodeTrait {
    fn set_atts (&self, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag);
    fn draw (&self, draw_ctx: *const RsvgDrawingCtx, dominate: i32);
}

// strong Rc<Node> references in the toplevel RsvgHandle.all_nodes array
// weak references elsewhere inside of Node

pub struct Node<'a> {
    node_type: NodeType,
    parent:    Option<Weak<RefCell<Node<'a>>>>, // optional; weak ref to parent-made-mutable
    children:  Vec<Rc<RefCell<Node<'a>>>>,   // strong references to children-made-mutable through RefCell
    state:     *mut RsvgState,
    node_impl: &'a NodeTrait
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

impl<'a> Node<'a> {
    pub fn new (node_type: NodeType,
                parent:    Option<Weak<RefCell<Node<'a>>>>,
                state:     *mut RsvgState,
                node_impl: &'a NodeTrait) -> Node<'a> {
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

    pub fn add_child (&mut self, child: &Rc<RefCell<Node<'a>>>) {
        self.children.push (child.clone ());
    }
}

pub enum RsvgNode {}

/* This is just to take a pointer to an Rc<RefCell<Node<'a>>> */
type RsvgRcNode<'a> = Rc<RefCell<Node<'a>>>;

#[no_mangle]
pub extern fn rsvg_node_get_type<'a> (raw_node: *const RsvgRcNode<'a>) -> NodeType {
    assert! (!raw_node.is_null ());
    let node: &RsvgRcNode<'a> = unsafe { & *raw_node };

    node.borrow ().get_type ()
}

#[no_mangle]
pub extern fn rsvg_node_get_parent<'a> (raw_node: *const RsvgRcNode<'a>) -> *const RsvgRcNode<'a> {
    assert! (!raw_node.is_null ());
    let rc_node: &RsvgRcNode<'a> = unsafe { & *raw_node };

    match rc_node.borrow ().parent {
        None => { ptr::null () }

        Some (ref weak_node) => {
            let strong_node = weak_node.upgrade ().unwrap ();
            &strong_node as *const RsvgRcNode<'a>
        }
    }
}

#[no_mangle]
pub extern fn rsvg_node_get_state<'a> (raw_node: *const RsvgRcNode<'a>) -> *mut RsvgState {
    assert! (!raw_node.is_null ());
    let rc_node: &RsvgRcNode<'a> = unsafe { & *raw_node };

    rc_node.borrow ().get_state ()
}

#[no_mangle]
pub extern fn rsvg_node_add_child<'a> (raw_node: *mut RsvgRcNode<'a>, raw_child: *const RsvgRcNode<'a>) {
    assert! (!raw_node.is_null ());
    assert! (!raw_child.is_null ());
    let rc_node: &mut RsvgRcNode<'a> = unsafe { &mut *raw_node };
    let rc_child: &RsvgRcNode<'a> = unsafe { & *raw_child };

    rc_node.borrow_mut ().add_child (rc_child);
}
