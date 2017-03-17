extern crate libc;

use drawing_ctx::RsvgDrawingCtx;
use drawing_ctx;
use handle::RsvgHandle;
use node::*;
use property_bag;
use property_bag::*;

/***** NodeGroup *****/

struct NodeGroup ();

impl NodeGroup {
    fn new () -> NodeGroup {
        NodeGroup ()
    }
}

impl NodeTrait for NodeGroup {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) -> NodeResult {
        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        node.draw_children (draw_ctx, dominate);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** NodeDefs *****/

struct NodeDefs ();

impl NodeDefs {
    fn new () -> NodeDefs {
        NodeDefs ()
    }
}

impl NodeTrait for NodeDefs {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) -> NodeResult {
        Ok (())
    }

    fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** NodeSwitch *****/

struct NodeSwitch ();

impl NodeSwitch {
    fn new () -> NodeSwitch {
        NodeSwitch ()
    }
}

impl NodeTrait for NodeSwitch {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) -> NodeResult {
        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        drawing_ctx::state_reinherit_top (draw_ctx, node.get_state (), dominate);

        drawing_ctx::push_discrete_layer (draw_ctx);

        for child in &*node.children.borrow () {
            if drawing_ctx::state_get_cond_true (child.get_state ()) {
                let boxed_child = box_node (child.clone ());

                drawing_ctx::draw_node_from_stack (draw_ctx, boxed_child, 0);

                rsvg_node_unref (boxed_child);

                break;
            }
        }

        drawing_ctx::pop_discrete_layer (draw_ctx);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** C Prototypes *****/

#[no_mangle]
pub extern fn rsvg_node_group_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Group,
                    raw_parent,
                    Box::new (NodeGroup::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_defs_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Defs,
                    raw_parent,
                    Box::new (NodeDefs::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_switch_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Switch,
                    raw_parent,
                    Box::new (NodeSwitch::new ()))
}
