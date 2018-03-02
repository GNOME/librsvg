#![cfg_attr(feature = "cargo-clippy", allow(clone_on_ref_ptr))]
#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]
#![cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]

extern crate cairo;
extern crate cairo_sys;
extern crate cssparser;
extern crate glib;
extern crate glib_sys;
extern crate itertools;
extern crate libc;
extern crate pango;
extern crate pango_sys;
extern crate regex;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate downcast_rs;

pub use attributes::rsvg_attribute_from_name;

pub use bbox::{rsvg_bbox_clip, rsvg_bbox_init, rsvg_bbox_insert, RsvgBbox};

pub use chars::{rsvg_node_chars_append, rsvg_node_chars_get_string, rsvg_node_chars_new};

pub use clip_path::{rsvg_node_clip_path_get_units, rsvg_node_clip_path_new};

pub use cnode::{rsvg_rust_cnode_get_impl, rsvg_rust_cnode_new};

pub use color::{rsvg_css_parse_color, AllowCurrentColor, AllowInherit, ColorKind, ColorSpec};

pub use gradient::{rsvg_node_linear_gradient_new, rsvg_node_radial_gradient_new};

pub use length::{
    rsvg_length_hand_normalize,
    rsvg_length_normalize,
    rsvg_length_parse,
    rsvg_parse_stroke_dasharray,
    LengthDir,
    LengthUnit,
    RsvgLength,
};

pub use image::rsvg_node_image_new;

pub use link::rsvg_node_link_new;

pub use marker::rsvg_node_marker_new;

pub use mask::{
    rsvg_node_mask_get_content_units,
    rsvg_node_mask_get_height,
    rsvg_node_mask_get_units,
    rsvg_node_mask_get_width,
    rsvg_node_mask_get_x,
    rsvg_node_mask_get_y,
    rsvg_node_mask_new,
};

pub use node::{
    rsvg_node_add_child,
    rsvg_node_draw,
    rsvg_node_draw_children,
    rsvg_node_foreach_child,
    rsvg_node_get_parent,
    rsvg_node_get_state,
    rsvg_node_get_type,
    rsvg_node_is_same,
    rsvg_node_ref,
    rsvg_node_set_attribute_parse_error,
    rsvg_node_set_atts,
    rsvg_node_unref,
};

pub use opacity::{rsvg_css_parse_opacity, OpacityKind, OpacitySpec};

pub use paint_server::{
    _set_source_rsvg_paint_server,
    rsvg_paint_server_parse,
    rsvg_paint_server_ref,
    rsvg_paint_server_unref,
};

pub use parsers::{rsvg_css_parse_number_list, rsvg_css_parse_number_optional_number};

pub use path_builder::rsvg_path_builder_add_to_cairo_context;

pub use pattern::rsvg_node_pattern_new;

pub use property_bag::{
    rsvg_property_bag_free,
    rsvg_property_bag_iter_begin,
    rsvg_property_bag_iter_end,
    rsvg_property_bag_iter_next,
    rsvg_property_bag_new,
};

pub use shapes::{
    rsvg_node_circle_new,
    rsvg_node_ellipse_new,
    rsvg_node_line_new,
    rsvg_node_path_new,
    rsvg_node_polygon_new,
    rsvg_node_polyline_new,
    rsvg_node_rect_new,
};

pub use space::rsvg_xml_space_normalize;

pub use stop::rsvg_node_stop_new;

pub use structure::{
    rsvg_node_defs_new,
    rsvg_node_group_new,
    rsvg_node_svg_apply_atts,
    rsvg_node_svg_get_size,
    rsvg_node_svg_get_view_box,
    rsvg_node_svg_new,
    rsvg_node_switch_new,
    rsvg_node_symbol_new,
    rsvg_node_use_new,
};

pub use text::rsvg_text_create_layout;

pub use transform::rsvg_parse_transform;

pub use viewbox::RsvgViewBox;

#[macro_use]
mod coord_units;

mod aspect_ratio;
mod attributes;
mod bbox;
#[cfg_attr(rustfmt, rustfmt_skip)]
mod chars;
mod clip_path;
mod cnode;
mod color;
mod drawing_ctx;
mod error;
mod gradient;
mod handle;
mod image;
mod length;
mod link;
mod marker;
mod mask;
mod node;
mod opacity;
mod paint_server;
mod parsers;
mod path_builder;
mod path_parser;
mod pattern;
mod property_bag;
mod shapes;
mod space;
mod state;
mod stop;
mod structure;
mod text;
mod transform;
mod util;
mod viewbox;
mod viewport;
