//! The <filter> node.
use std::cell::Cell;

use attributes::Attribute;
use coord_units::CoordUnits;
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;

/// The <filter> node.
pub struct NodeFilter {
    pub x: Cell<RsvgLength>,
    pub y: Cell<RsvgLength>,
    pub width: Cell<RsvgLength>,
    pub height: Cell<RsvgLength>,
    pub filterunits: Cell<CoordUnits>,
    pub primitiveunits: Cell<CoordUnits>,
}

impl NodeFilter {
    /// Constructs a new `NodeFilter` with default properties.
    #[inline]
    pub fn new() -> Self {
        Self {
            x: Cell::new(RsvgLength::parse("-10%", LengthDir::Horizontal).unwrap()),
            y: Cell::new(RsvgLength::parse("-10%", LengthDir::Vertical).unwrap()),
            width: Cell::new(RsvgLength::parse("120%", LengthDir::Horizontal).unwrap()),
            height: Cell::new(RsvgLength::parse("120%", LengthDir::Vertical).unwrap()),
            filterunits: Cell::new(CoordUnits::ObjectBoundingBox),
            primitiveunits: Cell::new(CoordUnits::UserSpaceOnUse),
        }
    }
}

impl NodeTrait for NodeFilter {
    fn set_atts(
        &self,
        _node: &RsvgNode,
        _handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical)?),
                Attribute::Width => self
                    .width
                    .set(parse("width", value, LengthDir::Horizontal)?),
                Attribute::Height => self
                    .height
                    .set(parse("height", value, LengthDir::Vertical)?),
                Attribute::FilterUnits => self.filterunits.set(parse("filterUnits", value, ())?),
                Attribute::PrimitiveUnits => {
                    self.primitiveunits.set(parse("primitiveUnits", value, ())?)
                }
                _ => (),
            }
        }

        Ok(())
    }
}
