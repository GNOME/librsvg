use std::cell::Cell;

use libc::c_char;

use attributes::Attribute;
use drawing_ctx::RsvgDrawingCtx;
use filter_context::FilterContext;
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{boxed_node_new, NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;
use state::State;

use super::{Filter, PrimitiveWithInput};

/// The `feOffset` filter primitive.
struct Offset {
    base: PrimitiveWithInput,
    dx: Cell<RsvgLength>,
    dy: Cell<RsvgLength>,
}

impl Offset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    fn new() -> Offset {
        Offset {
            base: PrimitiveWithInput::new::<Self>(),
            dx: Cell::new(RsvgLength::parse("0", LengthDir::Horizontal).unwrap()),
            dy: Cell::new(RsvgLength::parse("0", LengthDir::Vertical).unwrap()),
        }
    }
}

impl NodeTrait for Offset {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Dx => self.dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    #[inline]
    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: &State, _: i32, _: bool) {
        // Nothing; filters are drawn in rsvg-cairo-draw.c.
    }

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
    }
}

impl Filter for Offset {
    fn render(&self, ctx: &mut FilterContext) {
        let bounds = self.base.get_bounds(ctx);
        unimplemented!();
    }
}

/// Returns a new `feOffset` node.
#[no_mangle]
pub unsafe extern "C" fn rsvg_new_filter_primitive_offset(
    _element_name: *const c_char,
    parent: *mut RsvgNode,
) -> *mut RsvgNode {
    let filter = Offset::new();
    boxed_node_new(NodeType::FilterPrimitiveOffset, parent, Box::new(filter))
}
