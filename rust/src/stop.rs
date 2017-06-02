use ::libc;
use ::cssparser;
use ::glib::translate::*;

use std::cell::Cell;

use color::*;
use drawing_ctx;
use drawing_ctx::*;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use opacity::*;
use property_bag;
use property_bag::*;
use state::RsvgState;

struct NodeStop {
    offset: Cell<f64>,
    rgba: Cell<u32>
}

impl NodeStop {
    fn new () -> NodeStop {
        NodeStop {
            offset: Cell::new (0.0),
            rgba: Cell::new (0)
        }
    }

    pub fn get_offset (&self) -> f64 {
        self.offset.get ()
    }

    pub fn get_rgba (&self) -> u32 {
        self.rgba.get ()
    }
}

impl NodeTrait for NodeStop {
    fn set_atts (&self, node: &RsvgNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        let offset_length = property_bag::length_or_default (pbag, "offset", LengthDir::Both)?;
        match offset_length.unit {
            LengthUnit::Default |
            LengthUnit::Percent => {
                let mut offset = offset_length.length;

                if offset < 0.0 {
                    offset = 0.0;
                } else if offset > 1.0 {
                    offset = 1.0;
                }

                self.offset.set (offset);
            },

            _ => {
                return Err (NodeError::value_error ("offset", "stop offset must be in default or percent units"));
            }
        }

        let state = node.get_state ();

        // FIXME: this is the only place where rsvg_parse_style() and
        // rsvg_parse_style_pairs() are called outside of the
        // rsvg-base.c machinery.  That one indirectly calls them via
        // rsvg_parse_style_attrs().
        //
        // Should we resolve the stop-color / stop-opacity at
        // rendering time?

        if let Some (v) = property_bag::lookup (pbag, "style") {
            unsafe {
                rsvg_parse_style (handle, state, v.to_glib_none ().0);
            }
        }

        unsafe {
            rsvg_parse_style_pairs (state, pbag);
        }

        let inherited_state = drawing_ctx::state_new ();
        drawing_ctx::state_reconstruct (inherited_state, box_node (node.clone ()));

        let mut color_rgba: cssparser::RGBA;

        let stop_color = drawing_ctx::state_get_stop_color (state)
            .map_err (|e| NodeError::attribute_error ("stop-color", e))?;

        match stop_color {
            None => color_rgba = cssparser::RGBA::transparent (),

            Some (Color::Inherit) => {
                let inherited_stop_color = drawing_ctx::state_get_stop_color (inherited_state)
                    .map_err (|e| NodeError::attribute_error ("stop-color", e))?;

                match inherited_stop_color {
                    None => unreachable! (),

                    Some (Color::Inherit) => color_rgba = cssparser::RGBA::transparent (),
                    Some (Color::CurrentColor) => {
                        let color = drawing_ctx::state_get_current_color (inherited_state);
                        match color {
                            Color::RGBA (rgba) => color_rgba = rgba,
                            _ => unreachable! ()
                        }
                    },
                    
                    Some (Color::RGBA (rgba)) => color_rgba = rgba
                }
            },

            Some (Color::CurrentColor) => {
                let color = drawing_ctx::state_get_current_color (inherited_state);
                match color {
                    Color::RGBA (rgba) => color_rgba = rgba,
                    _ => unreachable! ()
                }
            }

            Some (Color::RGBA (rgba)) => color_rgba = rgba
        }

        let stop_opacity = drawing_ctx::state_get_stop_opacity (state)
            .map_err (|e| NodeError::attribute_error ("stop-opacity", e))?;

        match stop_opacity {
            None => color_rgba.alpha = 0xff,

            Some (Opacity::Inherit) => {
                let inherited_opacity = drawing_ctx::state_get_stop_opacity (inherited_state)
                    .map_err (|e| NodeError::attribute_error ("stop-opacity", e))?;

                match inherited_opacity {
                    Some (Opacity::Specified (opacity)) => color_rgba.alpha = opacity_to_u8 (opacity),
                    _ => color_rgba.alpha = 0xff
                }
            },

            Some (Opacity::Specified (opacity)) => color_rgba.alpha = opacity_to_u8 (opacity)
        }

        self.rgba.set (u32_from_rgba (color_rgba));

        drawing_ctx::state_free (inherited_state);

        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

fn u32_from_rgba (rgba: cssparser::RGBA) -> u32 {
    ((rgba.red as u32) << 24) |
    ((rgba.green as u32) << 16) |
    ((rgba.blue as u32) << 8) |
    (rgba.alpha as u32)
}

extern "C" {
    fn rsvg_parse_style_pairs (state: *mut RsvgState, pbag: *const RsvgPropertyBag);
    fn rsvg_parse_style (handle: *const RsvgHandle, state: *mut RsvgState, string: *const libc::c_char);
}

#[no_mangle]
pub extern fn rsvg_node_stop_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Stop,
                    raw_parent,
                    Box::new (NodeStop::new ()))
}
