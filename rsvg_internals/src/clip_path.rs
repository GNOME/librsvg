use libc;
use std::cell::Cell;

use attributes::Attribute;
use coord_units::CoordUnits;
use drawing_ctx::RsvgDrawingCtx;
use handle::RsvgHandle;
use node::{boxed_node_new, NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;
use state::State;

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

struct NodeClipPath {
    units: Cell<ClipPathUnits>,
}

impl NodeClipPath {
    fn new() -> NodeClipPath {
        NodeClipPath {
            units: Cell::new(ClipPathUnits::default()),
        }
    }
}

impl NodeTrait for NodeClipPath {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::ClipPathUnits => {
                    self.units.set(parse("clipPathUnits", value, (), None)?)
                }

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: &State, _: i32, _: bool) {
        // nothing; clip paths are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_clip_path_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::ClipPath,
        raw_parent,
        Box::new(NodeClipPath::new()),
    )
}

#[no_mangle]
pub extern "C" fn rsvg_node_clip_path_get_units(raw_node: *const RsvgNode) -> CoordUnits {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    let mut units = ClipPathUnits::default();

    node.with_impl(|clip_path: &NodeClipPath| {
        units = clip_path.units.get();
    });

    CoordUnits::from(units)
}
