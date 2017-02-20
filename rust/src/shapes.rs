use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;
use std::cell::Cell;
extern crate libc;

use cnode::*;
use drawing_ctx;
use drawing_ctx::*;
use handle::RsvgHandle;
use length::*;
use marker;
use node::*;
use path_builder::*;
use path_parser;
use property_bag;
use property_bag::*;
use state::RsvgState;

fn render_path_builder (builder:  &RsvgPathBuilder,
                        draw_ctx: *const RsvgDrawingCtx,
                        state:    *mut RsvgState,
                        dominate: i32,
                        render_markers: bool) {
    drawing_ctx::state_reinherit_top (draw_ctx, state, dominate);
    drawing_ctx::render_path_builder (draw_ctx, builder);

    if render_markers {
        marker::render_markers_for_path_builder (builder, draw_ctx);
    }
}

/***** NodePath *****/

struct NodePath {
    builder: RefCell<RsvgPathBuilder>
}

impl NodePath {
    fn new () -> NodePath {
        NodePath {
            builder: RefCell::new (RsvgPathBuilder::new ())
        }
    }
}

impl NodeTrait for NodePath {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        if let Some (value) = property_bag::lookup (pbag, "d") {
            let mut builder = self.builder.borrow_mut ();

            if let Err (_) = path_parser::parse_path_into_builder (&value, &mut *builder) {
                // FIXME: we don't propagate errors upstream, but creating a partial
                // path is OK per the spec
            }
        }
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        render_path_builder (&*self.builder.borrow (), draw_ctx, node.get_state (), dominate, true);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        ptr::null ()
    }
}

/***** NodePoly *****/
/*
struct NodePoly {
    builder: RefCell<RsvgPathBuilder>
}

impl NodePoly {
    fn new () -> NodePoly {
        NodePoly {
            builder: RefCell::new (RsvgPathBuilder::new ())
        }
    }
}

impl NodeTrait for NodePoly {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        // support for svg < 1.0 which used verts
        if let Some (value) = property_bag::lookup (pbag, "verts").or (property_bag::lookup (pbag, "points")) {
            let mut builder = self.builder.borrow_mut ();


        }
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        render_path_builder (&*self.builder.borrow (), draw_ctx, node.get_state (), dominate, true);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        ptr::null ()
    }
}
*/

/***** NodeLine *****/

struct NodeLine {
    x1: Cell<RsvgLength>,
    y1: Cell<RsvgLength>,
    x2: Cell<RsvgLength>,
    y2: Cell<RsvgLength>
}

impl NodeLine {
    fn new () -> NodeLine {
        NodeLine {
            x1: Cell::new (RsvgLength::default ()),
            y1: Cell::new (RsvgLength::default ()),
            x2: Cell::new (RsvgLength::default ()),
            y2: Cell::new (RsvgLength::default ())
        }
    }
}

impl NodeTrait for NodeLine {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        self.x1.set (property_bag::lookup (pbag, "x1").map_or (RsvgLength::default (),
                                                               |v| RsvgLength::parse (&v, LengthDir::Horizontal)));

        self.y1.set (property_bag::lookup (pbag, "y1").map_or (RsvgLength::default (),
                                                               |v| RsvgLength::parse (&v, LengthDir::Vertical)));

        self.x2.set (property_bag::lookup (pbag, "x2").map_or (RsvgLength::default (),
                                                               |v| RsvgLength::parse (&v, LengthDir::Horizontal)));

        self.y2.set (property_bag::lookup (pbag, "y2").map_or (RsvgLength::default (),
                                                               |v| RsvgLength::parse (&v, LengthDir::Vertical)));
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        let mut builder = RsvgPathBuilder::new ();

        let x1 = self.x1.get ().normalize (draw_ctx);
        let y1 = self.y1.get ().normalize (draw_ctx);
        let x2 = self.x2.get ().normalize (draw_ctx);
        let y2 = self.y2.get ().normalize (draw_ctx);

        builder.move_to (x1, y1);
        builder.line_to (x2, y2);

        render_path_builder (&builder, draw_ctx, node.get_state (), dominate, true);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        ptr::null ()
    }
}

/***** C Prototypes *****/

#[no_mangle]
pub extern fn rsvg_node_path_new (element_name: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    box_node (Rc::new (Node::new (NodeType::Path,
                                  parent_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  Box::new (NodePath::new ()))))
}

#[no_mangle]
pub extern fn rsvg_node_line_new (element_name: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    box_node (Rc::new (Node::new (NodeType::Line,
                                  parent_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  Box::new (NodeLine::new ()))))
}
