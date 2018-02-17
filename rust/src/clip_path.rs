use libc;
use std::cell::Cell;
use std::str::FromStr;

use attributes::Attribute;
use drawing_ctx::RsvgDrawingCtx;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode, boxed_node_new};
use coord_units::CoordUnits;
use parsers::parse;
use property_bag::PropertyBag;

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

struct NodeClipPath {
    units: Cell<ClipPathUnits>
}

impl NodeClipPath {
    fn new() -> NodeClipPath {
        NodeClipPath {
            units: Cell::new(ClipPathUnits::default())
        }
    }
}

impl NodeTrait for NodeClipPath {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (key, value) in pbag.iter() {
            if let Ok(attr) = Attribute::from_str(key) {
                match attr {
                    Attribute::ClipPathUnits =>
                        self.units.set(parse("clipPathUnits", value, (), None)?),

                    _ => (),
                }
            }
        }

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
pub extern fn rsvg_node_clip_path_get_units(raw_node: *const RsvgNode) -> CoordUnits {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut units = ClipPathUnits::default();

    node.with_impl(|clip_path: &NodeClipPath| {
        units = clip_path.units.get();
    });

    CoordUnits::from(units)
}
