use libc;
use std::cell::Cell;

use coord_units::CoordUnits;
use drawing_ctx::RsvgDrawingCtx;
use handle::RsvgHandle;
use length::{RsvgLength, LengthDir};
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode, boxed_node_new};
use parsers::Parse;
use property_bag::{self, PropertyBag};

coord_units!(MaskUnits, CoordUnits::ObjectBoundingBox);
coord_units!(MaskContentUnits, CoordUnits::UserSpaceOnUse);

struct NodeMask {
    x:      Cell<RsvgLength>,
    y:      Cell<RsvgLength>,
    width:  Cell<RsvgLength>,
    height: Cell<RsvgLength>,

    units:         Cell<MaskUnits>,
    content_units: Cell<MaskContentUnits>,
}

impl NodeMask {
    fn new() -> NodeMask {
        NodeMask {
            x:      Cell::new(NodeMask::get_default_pos(LengthDir::Horizontal)),
            y:      Cell::new(NodeMask::get_default_pos(LengthDir::Vertical)),

            width:  Cell::new(NodeMask::get_default_size(LengthDir::Horizontal)),
            height: Cell::new(NodeMask::get_default_size(LengthDir::Vertical)),

            units:         Cell::new(MaskUnits::default()),
            content_units: Cell::new(MaskContentUnits::default())
        }
    }

    fn get_default_pos(dir: LengthDir) -> RsvgLength {
        RsvgLength::parse("-10%", dir).unwrap()
    }

    fn get_default_size(dir: LengthDir) -> RsvgLength {
        RsvgLength::parse("120%", dir).unwrap()
    }
}

impl NodeTrait for NodeMask {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        self.x.set(property_bag::parse_or_value(pbag, "x",
                                                LengthDir::Horizontal,
                                                NodeMask::get_default_pos(LengthDir::Horizontal),
                                                None)?);
        self.y.set(property_bag::parse_or_value(pbag, "y",
                                                LengthDir::Vertical,
                                                NodeMask::get_default_pos(LengthDir::Vertical),
                                                None)?);

        self.width.set (property_bag::parse_or_value (pbag, "width",
                                                      LengthDir::Horizontal,
                                                      NodeMask::get_default_size(LengthDir::Horizontal),
                                                      Some(RsvgLength::check_nonnegative))?);
        self.height.set (property_bag::parse_or_value (pbag, "height",
                                                      LengthDir::Vertical,
                                                      NodeMask::get_default_size(LengthDir::Vertical),
                                                      Some(RsvgLength::check_nonnegative))?);

        self.units.set(property_bag::parse_or_default(pbag, "maskUnits", (), None)?);
        self.content_units.set(property_bag::parse_or_default(pbag, "maskContentUnits", (), None)?);

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing; masks are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern fn rsvg_node_mask_new(_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new(NodeType::Mask,
                   raw_parent,
                   Box::new(NodeMask::new()))
}

#[no_mangle]
pub extern fn rsvg_node_mask_get_x(raw_node: *const RsvgNode) -> RsvgLength {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut v = RsvgLength::default();

    node.with_impl(|mask: &NodeMask| {
        v = mask.x.get();
    });

    v
}

#[no_mangle]
pub extern fn rsvg_node_mask_get_y(raw_node: *const RsvgNode) -> RsvgLength {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut v = RsvgLength::default();

    node.with_impl(|mask: &NodeMask| {
        v = mask.y.get();
    });

    v
}

#[no_mangle]
pub extern fn rsvg_node_mask_get_width(raw_node: *const RsvgNode) -> RsvgLength {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut v = RsvgLength::default();

    node.with_impl(|mask: &NodeMask| {
        v = mask.width.get();
    });

    v
}

#[no_mangle]
pub extern fn rsvg_node_mask_get_height(raw_node: *const RsvgNode) -> RsvgLength {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut v = RsvgLength::default();

    node.with_impl(|mask: &NodeMask| {
        v = mask.height.get();
    });

    v
}

#[no_mangle]
pub extern fn rsvg_node_mask_get_units(raw_node: *const RsvgNode) -> CoordUnits {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut units = MaskUnits::default();

    node.with_impl(|mask: &NodeMask| {
        units = mask.units.get();
    });

    CoordUnits::from(units)
}

#[no_mangle]
pub extern fn rsvg_node_mask_get_content_units(raw_node: *const RsvgNode) -> CoordUnits {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut units = MaskContentUnits::default();

    node.with_impl(|mask: &NodeMask| {
        units = mask.content_units.get();
    });

    CoordUnits::from(units)
}
