//! The `clipPath` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::coord_units::CoordUnits;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

#[derive(Default)]
pub struct ClipPath {
    units: ClipPathUnits,
}

impl ClipPath {
    pub fn get_units(&self) -> CoordUnits {
        CoordUnits::from(self.units)
    }
}

impl NodeTrait for ClipPath {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "clipPathUnits") => self.units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}
