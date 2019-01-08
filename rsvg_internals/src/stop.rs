use std::cell::Cell;

use attributes::Attribute;
use error::*;
use handle::LoadOptions;
use length::*;
use node::*;
use parsers::ParseValue;
use property_bag::PropertyBag;
use unit_interval::UnitInterval;

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

fn validate_offset(length: Length) -> Result<Length, ValueErrorKind> {
    match length.unit {
        LengthUnit::Default | LengthUnit::Percent => Ok(length),
        _ => Err(ValueErrorKind::Value(
            "stop offset must be in default or percent units".to_string(),
        )),
    }
}

impl NodeTrait for NodeStop {
    fn set_atts(&self, _: &RsvgNode, _: &LoadOptions, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::Offset => {
                    self.offset.set(
                        attr.parse_and_validate(value, LengthDir::Both, validate_offset)
                            .map(|l| UnitInterval::clamp(l.length))?,
                    );
                }
                _ => (),
            }
        }

        Ok(())
    }
}
