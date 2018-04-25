use cssparser;
use glib::translate::*;
use glib_sys;
use libc;

use std::cell::Cell;

use attributes::Attribute;
use color::*;
use drawing_ctx::RsvgDrawingCtx;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use opacity::*;
use parsers::{parse, ParseError};
use property_bag::PropertyBag;
use state::{self, RsvgState};

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
        let state = node.get_state();

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
                    // FIXME: this is the only place where rsvg_parse_style_attribute_contents() and
                    // rsvg_parse_presentation_attributes() are called outside of the
                    // rsvg-base.c machinery.  That one indirectly calls them via
                    // rsvg_parse_style_attrs().
                    //
                    // Should we resolve the stop-color / stop-opacity at
                    // rendering time?

                    unsafe {
                        let success: bool = from_glib(rsvg_parse_style_attribute_contents(
                            state,
                            value.to_glib_none().0,
                        ));

                        if !success {
                            return Err(NodeError::parse_error(
                                "style",
                                ParseError::new("could not parse style"),
                            ));
                        }
                    }
                }

                _ => (),
            }
        }

        unsafe {
            rsvg_parse_presentation_attributes(state, pbag.ffi());
        }

        let inherited_state = state::new();
        state::reconstruct(inherited_state, node);

        let mut color_rgba: cssparser::RGBA;

        let stop_color =
            state::get_stop_color(state).map_err(|e| NodeError::attribute_error("stop-color", e))?;

        let current_color = state::get_state_rust(inherited_state)
            .color
            .as_ref()
            .map_or_else(|| state::Color::default().0, |c| c.0);

        match stop_color {
            None => color_rgba = cssparser::RGBA::transparent(),

            Some(Color::Inherit) => {
                let inherited_stop_color = state::get_stop_color(inherited_state)
                    .map_err(|e| NodeError::attribute_error("stop-color", e))?;

                match inherited_stop_color {
                    None => unreachable!(),

                    Some(Color::Inherit) => color_rgba = cssparser::RGBA::transparent(),
                    Some(Color::CurrentColor) => color_rgba = current_color,
                    Some(Color::RGBA(rgba)) => color_rgba = rgba,
                }
            }

            Some(Color::CurrentColor) => color_rgba = current_color,

            Some(Color::RGBA(rgba)) => color_rgba = rgba,
        }

        let stop_opacity = state::get_stop_opacity(state)
            .map_err(|e| NodeError::attribute_error("stop-opacity", e))?;

        match stop_opacity {
            None => color_rgba.alpha = 0xff,

            Some(Opacity::Inherit) => {
                let inherited_opacity = state::get_stop_opacity(inherited_state)
                    .map_err(|e| NodeError::attribute_error("stop-opacity", e))?;

                match inherited_opacity {
                    Some(Opacity::Specified(opacity)) => color_rgba.alpha = opacity_to_u8(opacity),
                    _ => color_rgba.alpha = 0xff,
                }
            }

            Some(Opacity::Specified(opacity)) => color_rgba.alpha = opacity_to_u8(opacity),
        }

        self.rgba.set(u32_from_rgba(color_rgba));

        state::free(inherited_state);

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

fn u32_from_rgba(rgba: cssparser::RGBA) -> u32 {
    (u32::from(rgba.red) << 24) | (u32::from(rgba.green) << 16) | (u32::from(rgba.blue) << 8)
        | u32::from(rgba.alpha)
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_parse_presentation_attributes(state: *mut RsvgState, pbag: *const PropertyBag);
    fn rsvg_parse_style_attribute_contents(
        state: *mut RsvgState,
        string: *const libc::c_char,
    ) -> glib_sys::gboolean;
}

#[no_mangle]
pub extern "C" fn rsvg_node_stop_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Stop, raw_parent, Box::new(NodeStop::new()))
}
