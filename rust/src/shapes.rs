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

/***** NodeRect *****/

struct NodeRect {
    // x, y, width, height
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    w: Cell<RsvgLength>,
    h: Cell<RsvgLength>,

    // Radiuses for rounded corners
    rx: Cell<Option<RsvgLength>>,
    ry: Cell<Option<RsvgLength>>
}

impl NodeRect {
    fn new () -> NodeRect {
        NodeRect {
            x: Cell::new (RsvgLength::default ()),
            y: Cell::new (RsvgLength::default ()),
            w: Cell::new (RsvgLength::default ()),
            h: Cell::new (RsvgLength::default ()),

            rx: Cell::new (None),
            ry: Cell::new (None)
        }
    }
}

impl NodeTrait for NodeRect {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) {
        self.x.set (property_bag::lookup (pbag, "x").map_or (RsvgLength::default (),
                                                             |v| RsvgLength::parse (&v, LengthDir::Horizontal)));

        self.y.set (property_bag::lookup (pbag, "y").map_or (RsvgLength::default (),
                                                             |v| RsvgLength::parse (&v, LengthDir::Vertical)));

        self.w.set (property_bag::lookup (pbag, "width").map_or (RsvgLength::default (),
                                                                 |v| RsvgLength::parse (&v, LengthDir::Horizontal)));

        self.h.set (property_bag::lookup (pbag, "height").map_or (RsvgLength::default (),
                                                                  |v| RsvgLength::parse (&v, LengthDir::Vertical)));

        self.rx.set (property_bag::lookup (pbag, "rx").map_or (None,
                                                               |v| Some (RsvgLength::parse (&v, LengthDir::Horizontal))));

        self.ry.set (property_bag::lookup (pbag, "ry").map_or (None,
                                                               |v| Some (RsvgLength::parse (&v, LengthDir::Vertical))));
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        let x = self.x.get ().normalize (draw_ctx);
        let y = self.y.get ().normalize (draw_ctx);

        let w = self.w.get ().normalize (draw_ctx);
        let h = self.h.get ().normalize (draw_ctx);

        let mut rx;
        let mut ry;

        match (self.rx.get (), self.ry.get ()) {
            (None, None) => {
                rx = 0.0;
                ry = 0.0;
            },

            (Some (_rx), None) => {
                rx = _rx.normalize (draw_ctx);
                ry = _rx.normalize (draw_ctx);
            },

            (None, Some (_ry)) => {
                rx = _ry.normalize (draw_ctx);
                ry = _ry.normalize (draw_ctx);
            },

            (Some (_rx), Some (_ry)) => {
                rx = _rx.normalize (draw_ctx);
                ry = _ry.normalize (draw_ctx);
            }
        }

        // Per the spec, w,h must be >= 0
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // ... and rx,ry must be nonnegative
        if rx < 0.0 || ry < 0.0 {
            return;
        }

        let half_w = w / 2.0;
        let half_h = h / 2.0;

        if rx > half_w {
            rx = half_w;
        }

        if ry > half_h {
            ry = half_h;
        }

        if rx == 0.0 {
            ry = 0.0;
        } else if ry == 0.0 {
            rx = 0.0;
        }

        let mut builder = RsvgPathBuilder::new ();

        if rx == 0.0 {
            /* Easy case, no rounded corners */

            builder.move_to (x, y);
            builder.line_to (x + w, y);
            builder.line_to (x + w, y + h);
            builder.line_to (x, y + h);
            builder.line_to (x, y);
            builder.close_path ();
        } else {
            /* Hard case, rounded corners
             *
             *      (top_x1, top_y)                   (top_x2, top_y)
             *     *--------------------------------*
             *    /                                  \
             *   * (left_x, left_y1)                  * (right_x, right_y1)
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   * (left_x, left_y2)                  * (right_x, right_y2)
             *    \                                  /
             *     *--------------------------------*
             *      (bottom_x1, bottom_y)            (bottom_x2, bottom_y)
             */

            let top_x1 = x + rx;
            let top_x2 = x + w - rx;
            let top_y  = y;

            let bottom_x1 = top_x1;
            let bottom_x2 = top_x2;
            let bottom_y  = y + h;

            let left_x  = x;
            let left_y1 = y + ry;
            let left_y2 = y + h - ry;

            let right_x  = x + w;
            let right_y1 = left_y1;
            let right_y2 = left_y2;

            builder.move_to (top_x1, top_y);
            builder.line_to (top_x2, top_y);

            builder.arc (top_x2, top_y,
                         rx, ry, 0.0, false, true,
                         right_x, right_y1);

            builder.line_to (right_x, right_y2);

            builder.arc (right_x, right_y2,
                         rx, ry, 0.0, false, true,
                         bottom_x2, bottom_y);

            builder.line_to (bottom_x1, bottom_y);

            builder.arc (bottom_x1, bottom_y,
                         rx, ry, 0.0, false, true,
                         left_x, left_y2);

            builder.line_to (left_x, left_y1);

            builder.arc (left_x, left_y1,
                         rx, ry, 0.0, false, true,
                         top_x1, top_y);

            builder.close_path ();
        }

        render_path_builder (&builder, draw_ctx, node.get_state (), dominate, false);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        ptr::null ()
    }
}

/***** C Prototypes *****/

#[no_mangle]
pub extern fn rsvg_node_path_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    box_node (Rc::new (Node::new (NodeType::Path,
                                  parent_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  Box::new (NodePath::new ()))))
}

#[no_mangle]
pub extern fn rsvg_node_line_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    box_node (Rc::new (Node::new (NodeType::Line,
                                  parent_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  Box::new (NodeLine::new ()))))
}

#[no_mangle]
pub extern fn rsvg_node_rect_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    box_node (Rc::new (Node::new (NodeType::Rect,
                                  parent_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  Box::new (NodeRect::new ()))))
}
