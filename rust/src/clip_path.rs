use libc;
use std::cell::Cell;

use drawing_ctx::RsvgDrawingCtx;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode, boxed_node_new};
use paint_server::PaintServerUnits;
use pattern::PatternContentUnits;
use property_bag::{self, RsvgPropertyBag};

type ClipPathUnits = PatternContentUnits;

struct NodeClipPath {
    units: Cell<ClipPathUnits>
}

impl NodeClipPath {
    fn new() -> NodeClipPath {
        NodeClipPath {
            units: Cell::new(PatternContentUnits::from(PaintServerUnits::UserSpaceOnUse))
        }
    }
}

impl NodeTrait for NodeClipPath {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        self.units.set(property_bag::parse_or_default(pbag, "clipPathUnits", (), None)?);

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing; clip paths are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern fn rsvg_node_clip_path_new(_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new(NodeType::ClipPath,
                   raw_parent,
                   Box::new(NodeClipPath::new()))
}

#[no_mangle]
pub extern fn rsvg_node_clip_path_get_units(raw_node: *const RsvgNode) -> PaintServerUnits {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut units = PatternContentUnits::default();

    node.with_impl(|clip_path: &NodeClipPath| {
        units = clip_path.units.get();
    });

    units.0
}
