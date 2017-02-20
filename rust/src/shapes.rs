use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;
extern crate libc;

use cnode::*;
use drawing_ctx;
use drawing_ctx::*;
use handle::RsvgHandle;
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

/***** C Prototypes *****/

#[no_mangle]
pub extern fn rsvg_node_path_new (element_name: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    box_node (Rc::new (Node::new (NodeType::Path,
                                  parent_ptr_to_weak (raw_parent),
                                  drawing_ctx::state_new (),
                                  Box::new (NodePath::new ()))))
}
