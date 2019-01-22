//! The <filter> node.
use std::cell::Cell;

use attributes::Attribute;
use coord_units::CoordUnits;
use error::ValueErrorKind;
use length::{LengthHorizontal, LengthUnit, LengthVertical};
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{Parse, ParseError, ParseValue};
use property_bag::PropertyBag;

/// The <filter> node.
pub struct NodeFilter {
    pub x: Cell<LengthHorizontal>,
    pub y: Cell<LengthVertical>,
    pub width: Cell<LengthHorizontal>,
    pub height: Cell<LengthVertical>,
    pub filterunits: Cell<CoordUnits>,
    pub primitiveunits: Cell<CoordUnits>,
}

impl NodeFilter {
    /// Constructs a new `NodeFilter` with default properties.
    #[inline]
    pub fn new() -> Self {
        Self {
            x: Cell::new(LengthHorizontal::parse_str("-10%", ()).unwrap()),
            y: Cell::new(LengthVertical::parse_str("-10%", ()).unwrap()),
            width: Cell::new(LengthHorizontal::parse_str("120%", ()).unwrap()),
            height: Cell::new(LengthVertical::parse_str("120%", ()).unwrap()),
            filterunits: Cell::new(CoordUnits::ObjectBoundingBox),
            primitiveunits: Cell::new(CoordUnits::UserSpaceOnUse),
        }
    }
}

impl NodeTrait for NodeFilter {
    fn set_atts(&self, _node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        // Parse filterUnits first as it affects x, y, width, height checks.
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::FilterUnits => self.filterunits.set(attr.parse(value, ())?),
                _ => (),
            }
        }

        // With ObjectBoundingBox, only fractions and percents are allowed.
        let no_units_allowed = self.filterunits.get() == CoordUnits::ObjectBoundingBox;

        let check_units_horizontal = |length: LengthHorizontal| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit() {
                LengthUnit::Default | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
            }
        };

        let check_units_vertical = |length: LengthVertical| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit() {
                LengthUnit::Default | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
            }
        };

        let check_units_horizontal_and_ensure_nonnegative = |length: LengthHorizontal| {
            check_units_horizontal(length).and_then(LengthHorizontal::check_nonnegative)
        };

        let check_units_vertical_and_ensure_nonnegative = |length: LengthVertical| {
            check_units_vertical(length).and_then(LengthVertical::check_nonnegative)
        };

        // Parse the rest of the attributes.
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X => {
                    self.x
                        .set(attr.parse_and_validate(value, (), check_units_horizontal)?)
                }
                Attribute::Y => {
                    self.y
                        .set(attr.parse_and_validate(value, (), check_units_vertical)?)
                }
                Attribute::Width => self.width.set(attr.parse_and_validate(
                    value,
                    (),
                    check_units_horizontal_and_ensure_nonnegative,
                )?),
                Attribute::Height => self.height.set(attr.parse_and_validate(
                    value,
                    (),
                    check_units_vertical_and_ensure_nonnegative,
                )?),
                Attribute::PrimitiveUnits => self.primitiveunits.set(attr.parse(value, ())?),
                _ => (),
            }
        }

        Ok(())
    }
}
