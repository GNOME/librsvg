use cssparser;
use libc;

use std::cell::Cell;

use attributes::Attribute;
use color::Color;
use drawing_ctx::RsvgDrawingCtx;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::parse;
use property_bag::PropertyBag;
use state::{self, ComputedValues, State, StopColor, StopOpacity};

pub struct NodeStop {
    offset: Cell<f64>,
    rgba: Cell<u32>,
}

impl NodeStop {
    fn new() -> NodeStop {
        NodeStop {
            offset: Cell::new(0.0),
            rgba: Cell::new(0),
        }
    }

    pub fn get_offset(&self) -> f64 {
        self.offset.get()
    }

    pub fn get_rgba(&self) -> u32 {
        self.rgba.get()
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
    fn set_atts(&self, node: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        let state = node.get_state_mut();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Offset => {
                    let length = parse("offset", value, LengthDir::Both, Some(validate_offset))?;
                    assert!(
                        length.unit == LengthUnit::Default || length.unit == LengthUnit::Percent
                    );
                    self.offset.set(length.length);
                }

                Attribute::Style => {
                    // FIXME: this is the only place where parse_style_declarations() and
                    // parse_presentation_attributes() are called outside of the
                    // rsvg-base.c machinery.  That one indirectly calls them via
                    // rsvg_parse_style_attrs().
                    //
                    // Should we resolve the stop-color / stop-opacity at
                    // rendering time?

                    state.parse_style_declarations(value)?;
                }

                _ => (),
            }
        }

        state.parse_presentation_attributes(pbag)?;

        let mut inherited_state = State::new_with_parent(None);
        inherited_state.reconstruct(node);

        let mut color_rgba: cssparser::RGBA;

        let current_color = inherited_state
            .color
            .as_ref()
            .map_or_else(|| state::Color::default().0, |c| c.0);

        match state.stop_color {
            None => match inherited_state.stop_color {
                None => color_rgba = cssparser::RGBA::transparent(),
                Some(StopColor(Color::CurrentColor)) => color_rgba = current_color,
                Some(StopColor(Color::RGBA(rgba))) => color_rgba = rgba,
            },

            Some(StopColor(Color::CurrentColor)) => color_rgba = current_color,

            Some(StopColor(Color::RGBA(rgba))) => color_rgba = rgba,
        }

        match state.stop_opacity {
            None => match inherited_state.stop_opacity {
                Some(StopOpacity(val)) => color_rgba.alpha = u8::from(val),
                _ => color_rgba.alpha = 0xff,
            },

            Some(StopOpacity(val)) => color_rgba.alpha = u8::from(val),
        }

        self.rgba.set(u32_from_rgba(color_rgba));

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: &ComputedValues, _: i32, _: bool) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

fn u32_from_rgba(rgba: cssparser::RGBA) -> u32 {
    (u32::from(rgba.red) << 24)
        | (u32::from(rgba.green) << 16)
        | (u32::from(rgba.blue) << 8)
        | u32::from(rgba.alpha)
}

#[no_mangle]
pub extern "C" fn rsvg_node_stop_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Stop, raw_parent, Box::new(NodeStop::new()))
}
