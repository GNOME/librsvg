//! The <filter> node.
use std::cell::Cell;

use attributes::Attribute;
use coord_units::CoordUnits;
use error::ValueErrorKind;
use handle::RsvgHandle;
use length::{Length, LengthDir, LengthUnit};
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{parse, parse_and_validate, Parse, ParseError};
use property_bag::PropertyBag;

/// The <filter> node.
pub struct NodeFilter {
    pub x: Cell<Length>,
    pub y: Cell<Length>,
    pub width: Cell<Length>,
    pub height: Cell<Length>,
    pub filterunits: Cell<CoordUnits>,
    pub primitiveunits: Cell<CoordUnits>,
}

impl NodeFilter {
    /// Constructs a new `NodeFilter` with default properties.
    #[inline]
    pub fn new() -> Self {
        Self {
            x: Cell::new(Length::parse_str("-10%", LengthDir::Horizontal).unwrap()),
            y: Cell::new(Length::parse_str("-10%", LengthDir::Vertical).unwrap()),
            width: Cell::new(Length::parse_str("120%", LengthDir::Horizontal).unwrap()),
            height: Cell::new(Length::parse_str("120%", LengthDir::Vertical).unwrap()),
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
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        // Parse filterUnits first as it affects x, y, width, height checks.
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::FilterUnits => self.filterunits.set(parse("filterUnits", value, ())?),
                _ => (),
            }
        }

        // With ObjectBoundingBox, only fractions and percents are allowed.
        let no_units_allowed = self.filterunits.get() == CoordUnits::ObjectBoundingBox;
        let check_units = |length: Length| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Default | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
            }
        };
        let check_units_and_ensure_nonnegative =
            |length: Length| check_units(length).and_then(Length::check_nonnegative);

        // Parse the rest of the attributes.
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse_and_validate(
                    "x",
                    value,
                    LengthDir::Horizontal,
                    check_units,
                )?),
                Attribute::Y => self.y.set(parse_and_validate(
                    "y",
                    value,
                    LengthDir::Vertical,
                    check_units,
                )?),
                Attribute::Width => self.width.set(parse_and_validate(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    check_units_and_ensure_nonnegative,
                )?),
                Attribute::Height => self.height.set(parse_and_validate(
                    "height",
                    value,
                    LengthDir::Vertical,
                    check_units_and_ensure_nonnegative,
                )?),
                Attribute::PrimitiveUnits => {
                    self.primitiveunits.set(parse("primitiveUnits", value, ())?)
                }
                _ => (),
            }
        }

        Ok(())
    }
}
