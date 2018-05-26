use libc;

use std::cell::Cell;

use attributes::Attribute;
use drawing_ctx::RsvgDrawingCtx;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::parse;
use property_bag::PropertyBag;
use state::ComputedValues;

pub struct NodeStop {
    offset: Cell<f64>,
}

impl NodeStop {
    fn new() -> NodeStop {
        NodeStop {
            offset: Cell::new(0.0),
        }
    }

    pub fn get_offset(&self) -> f64 {
        self.offset.get()
    }
}

fn validate_offset(length: RsvgLength) -> Result<RsvgLength, AttributeError> {
    match length.unit {
        LengthUnit::Default | LengthUnit::Percent => {
            let mut offset = length.length;

            if offset < 0.0 {
                offset = 0.0;
            } else if offset > 1.0 {
                offset = 1.0;
            }

            Ok(RsvgLength::new(
                offset,
                LengthUnit::Default,
                LengthDir::Both,
            ))
        }

        _ => Err(AttributeError::Value(
            "stop offset must be in default or percent units".to_string(),
        )),
    }
}

impl NodeTrait for NodeStop {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Offset => {
                    let length = parse("offset", value, LengthDir::Both, Some(validate_offset))?;
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

    fn draw(&self, _: &RsvgNode, _: &ComputedValues, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_stop_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Stop, raw_parent, Box::new(NodeStop::new()))
}
