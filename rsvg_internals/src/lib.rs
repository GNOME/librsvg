#![cfg_attr(feature = "cargo-clippy", allow(clone_on_ref_ptr))]
#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]
#![cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]

extern crate cairo;
extern crate cairo_sys;
extern crate cssparser;
extern crate float_cmp;
extern crate glib;
extern crate glib_sys;
extern crate itertools;
extern crate libc;
extern crate owning_ref;
extern crate pango;
extern crate pango_cairo_sys;
extern crate pango_sys;
extern crate pangocairo;
extern crate regex;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate downcast_rs;

#[macro_use]
extern crate rulinalg;

pub use attributes::rsvg_attribute_from_name;

pub use color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use drawing_ctx::{
    rsvg_drawing_ctx_add_node_and_ancestors_to_stack,
    rsvg_drawing_ctx_draw_node_from_stack,
    rsvg_drawing_ctx_free,
    rsvg_drawing_ctx_get_ink_rect,
    rsvg_drawing_ctx_new,
};

pub use length::{
    rsvg_length_hand_normalize,
    rsvg_length_normalize,
    rsvg_length_parse,
    LengthDir,
    LengthUnit,
    RsvgLength,
};

pub use load::rsvg_load_new_node;

pub use node::{
    rsvg_node_add_child,
    rsvg_node_children_iter_begin,
    rsvg_node_children_iter_end,
    rsvg_node_children_iter_next,
    rsvg_node_children_iter_next_back,
    rsvg_node_get_parent,
    rsvg_node_get_type,
    rsvg_node_is_same,
    rsvg_node_ref,
    rsvg_node_register_in_defs,
    rsvg_node_set_attribute_parse_error,
    rsvg_node_set_atts,
    rsvg_node_set_overridden_properties,
    rsvg_node_unref,
    rsvg_root_node_cascade,
};

pub use parsers::{rsvg_css_parse_number_list, rsvg_css_parse_number_optional_number};

pub use property_bag::{
    rsvg_property_bag_free,
    rsvg_property_bag_iter_begin,
    rsvg_property_bag_iter_end,
    rsvg_property_bag_iter_next,
    rsvg_property_bag_new,
};

pub use state::{
    rsvg_computed_values_get_lighting_color_argb,
    rsvg_parse_style_attrs,
    rsvg_state_free,
    rsvg_state_new,
    rsvg_state_parse_style_pair,
};

pub use structure::{rsvg_node_svg_apply_atts, rsvg_node_svg_get_size, rsvg_node_svg_get_view_box};

pub use text::{rsvg_node_chars_append, rsvg_node_chars_new};

#[macro_use]
mod coord_units;

#[macro_use]
mod float_eq_cairo;

#[macro_use]
mod property_macros;

mod aspect_ratio;
mod attributes;
mod bbox;
mod clip_path;
mod color;
mod cond;
mod defs;
mod drawing_ctx;
mod error;
pub mod filters;
mod gradient;
mod handle;
mod image;
mod iri;
mod length;
mod link;
mod load;
mod marker;
mod mask;
mod node;
mod paint_server;
mod parsers;
mod path_builder;
mod path_parser;
mod pattern;
mod property_bag;
mod rect;
mod shapes;
mod space;
mod srgb;
mod state;
mod stop;
mod structure;
pub mod surface_utils;
mod text;
mod transform;
mod unitinterval;
mod util;
mod viewbox;
mod viewport;
