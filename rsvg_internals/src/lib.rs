#![cfg_attr(feature = "cargo-clippy", allow(clone_on_ref_ptr))]
#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]
#![cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]

extern crate cairo;
extern crate cairo_sys;
extern crate cssparser;
extern crate float_cmp;
extern crate gdk_pixbuf;
extern crate glib;
extern crate glib_sys;
extern crate itertools;
extern crate language_tags;
extern crate libc;
extern crate locale_config;
extern crate nalgebra;
extern crate num_traits;
extern crate owning_ref;
extern crate pango;
extern crate pango_cairo_sys;
extern crate pango_sys;
extern crate pangocairo;
extern crate rayon;
extern crate regex;

#[macro_use]
extern crate lazy_static;

extern crate downcast_rs;

pub use color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use css::{rsvg_css_parse_into_handle, rsvg_css_styles_free, rsvg_css_styles_new};

pub use defs::{rsvg_defs_free, rsvg_defs_lookup, rsvg_defs_new};

pub use drawing_ctx::{
    rsvg_drawing_ctx_add_node_and_ancestors_to_stack,
    rsvg_drawing_ctx_draw_node_from_stack,
    rsvg_drawing_ctx_free,
    rsvg_drawing_ctx_get_ink_rect,
    rsvg_drawing_ctx_new,
};

pub use load::{rsvg_load_new_node, rsvg_load_set_node_atts, rsvg_load_set_svg_node_atts};

pub use node::{
    rsvg_node_add_child,
    rsvg_node_children_iter_begin,
    rsvg_node_children_iter_end,
    rsvg_node_children_iter_next,
    rsvg_node_find_last_chars_child,
    rsvg_node_get_parent,
    rsvg_node_ref,
    rsvg_node_unref,
};

pub use tree::{
    rsvg_tree_cascade,
    rsvg_tree_free,
    rsvg_tree_get_root,
    rsvg_tree_is_root,
    rsvg_tree_new,
    rsvg_tree_root_is_svg,
};

pub use property_bag::{
    rsvg_property_bag_free,
    rsvg_property_bag_iter_begin,
    rsvg_property_bag_iter_end,
    rsvg_property_bag_iter_next,
    rsvg_property_bag_new,
};

pub use structure::rsvg_node_svg_get_size;

pub use text::{rsvg_node_chars_append, rsvg_node_chars_new};

#[macro_use]
mod log;

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
mod croco;
mod css;
mod defs;
mod drawing_ctx;
mod error;
pub mod filters;
mod font_props;
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
pub mod srgb;
mod state;
mod stop;
mod structure;
pub mod surface_utils;
mod text;
mod transform;
mod tree;
mod unitinterval;
mod util;
mod viewbox;
mod viewport;
