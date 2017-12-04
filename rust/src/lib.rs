extern crate cairo;
extern crate cairo_sys;
extern crate cssparser;
extern crate glib;
extern crate glib_sys;
extern crate libc;

#[macro_use]
extern crate downcast_rs;

pub use bbox::{
    RsvgBbox,
    rsvg_bbox_init,
    rsvg_bbox_insert,
    rsvg_bbox_clip
};

pub use clip_path::{
    rsvg_node_clip_path_new,
    rsvg_node_clip_path_get_units
};

pub use cnode::{
    rsvg_rust_cnode_new,
    rsvg_rust_cnode_get_impl
};

pub use color::{
    AllowCurrentColor,
    AllowInherit,
    ColorKind,
    ColorSpec,
    rsvg_css_parse_color
};

pub use gradient::{
    rsvg_node_linear_gradient_new,
    rsvg_node_radial_gradient_new,
    gradient_resolve_fallbacks_and_set_pattern
};

pub use length::{
    LengthUnit,
    LengthDir,
    RsvgLength,
    rsvg_length_parse,
    rsvg_length_normalize,
    rsvg_length_hand_normalize,
};

pub use image::{
    rsvg_node_image_new,
};

pub use marker::{
    rsvg_node_marker_new,
};

pub use node::{
    rsvg_node_get_type,
    rsvg_node_get_parent,
    rsvg_node_ref,
    rsvg_node_unref,
    rsvg_node_is_same,
    rsvg_node_get_state,
    rsvg_node_add_child,
    rsvg_node_set_atts,
    rsvg_node_draw,
    rsvg_node_set_attribute_parse_error,
    rsvg_node_foreach_child,
    rsvg_node_draw_children,
};

pub use opacity::{
    OpacityKind,
    OpacitySpec,
    rsvg_css_parse_opacity
};

pub use parsers::{
    rsvg_css_parse_number_list,
    rsvg_css_parse_number_optional_number
};

pub use path_builder::{
    rsvg_path_builder_add_to_cairo_context
};

pub use pattern::{
    rsvg_node_pattern_new,
    pattern_resolve_fallbacks_and_set_pattern,
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

pub use stop::{
    rsvg_node_stop_new
};

pub use structure::{
    rsvg_node_group_new,
    rsvg_node_defs_new,
    rsvg_node_switch_new,
    rsvg_node_symbol_new,
    rsvg_node_svg_new,
    rsvg_node_svg_get_size,
    rsvg_node_svg_get_view_box,
    rsvg_node_svg_apply_atts,
    rsvg_node_use_new,
};

pub use transform::{
    rsvg_parse_transform,
};

pub use viewbox::{
    RsvgViewBox
};


#[macro_use]
mod coord_units;

mod aspect_ratio;
mod bbox;
mod clip_path;
mod cnode;
mod color;
mod drawing_ctx;
mod error;
mod gradient;
mod handle;
mod image;
mod length;
mod marker;
mod node;
mod opacity;
mod paint_server;
mod parse_transform;
mod parsers;
mod path_builder;
mod path_parser;
mod pattern;
mod property_bag;
mod shapes;
mod state;
mod stop;
mod structure;
mod transform;
mod util;
mod viewbox;
mod viewport;
