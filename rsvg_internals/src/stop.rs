use std::cell::Cell;

use attributes::Attribute;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::parse_and_validate;
use property_bag::PropertyBag;

pub struct NodeStop {
    offset: Cell<f64>,
}

impl NodeStop {
    pub fn new() -> NodeStop {
        NodeStop {
            offset: Cell::new(0.0),
        }
    }

    pub fn get_offset(&self) -> f64 {
        self.offset.get()
    }
}

fn validate_offset(length: Length) -> Result<Length, ValueErrorKind> {
    match length.unit {
        LengthUnit::Default | LengthUnit::Percent => {
            let mut offset = length.length;

            if offset < 0.0 {
                offset = 0.0;
            } else if offset > 1.0 {
                offset = 1.0;
            }

            Ok(Length::new(offset, LengthUnit::Default, LengthDir::Both))
        }

        _ => Err(ValueErrorKind::Value(
            "stop offset must be in default or percent units".to_string(),
        )),
    }
}

impl NodeTrait for NodeStop {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Offset => {
                    let length =
                        parse_and_validate("offset", value, LengthDir::Both, validate_offset)?;
                    assert!(
                        length.unit == LengthUnit::Default || length.unit == LengthUnit::Percent
                    );
                    self.offset.set(length.length);
                }

                _ => (),
            }
        }

        Ok(())
    }
}
