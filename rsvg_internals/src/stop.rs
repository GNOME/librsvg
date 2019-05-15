use std::cell::Cell;

use crate::error::*;
use crate::length::*;
use crate::node::*;
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;
use crate::unit_interval::UnitInterval;

pub struct NodeStop {
    offset: Cell<UnitInterval>,
}

impl NodeStop {
    pub fn new() -> NodeStop {
        NodeStop {
            offset: Cell::new(UnitInterval(0.0)),
        }
    }

    pub fn get_offset(&self) -> UnitInterval {
        self.offset.get()
    }
}

fn validate_offset(length: LengthBoth) -> Result<LengthBoth, ValueErrorKind> {
    match length.unit() {
        LengthUnit::Px | LengthUnit::Percent => Ok(length),
        _ => Err(ValueErrorKind::Value(
            "stop offset must be in default or percent units".to_string(),
        )),
    }
}

impl NodeTrait for NodeStop {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("offset") => {
                    self.offset.set(
                        attr.parse_and_validate(value, validate_offset)
                            .map(|l| UnitInterval::clamp(l.length()))?,
                    );
                }
                _ => (),
            }
        }

        Ok(())
    }
}
