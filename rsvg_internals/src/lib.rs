#![cfg_attr(feature = "cargo-clippy", allow(clone_on_ref_ptr))]
#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]
#![cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]

extern crate cairo;
extern crate cairo_sys;
extern crate cssparser;
extern crate data_url;
extern crate downcast_rs;
extern crate encoding;
extern crate float_cmp;
extern crate gdk_pixbuf;
extern crate gio;
extern crate gio_sys;
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
extern crate url;

#[macro_use]
extern crate lazy_static;

pub use color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use css::{rsvg_css_styles_free, rsvg_css_styles_new};

pub use defs::{rsvg_defs_free, rsvg_defs_lookup};

pub use drawing_ctx::{
    rsvg_drawing_ctx_add_node_and_ancestors_to_stack,
    rsvg_drawing_ctx_draw_node_from_stack,
    rsvg_drawing_ctx_free,
    rsvg_drawing_ctx_get_ink_rect,
    rsvg_drawing_ctx_new,
};

pub use handle::rsvg_handle_load_css;

pub use io::{
    rsvg_decode_data_uri,
    rsvg_get_input_stream_for_loading,
    rsvg_io_acquire_data,
    rsvg_io_acquire_stream,
};

pub use node::rsvg_node_unref;

pub use tree::{
    rsvg_tree_cascade,
    rsvg_tree_free,
    rsvg_tree_get_root,
    rsvg_tree_is_root,
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

pub use xml::{
    rsvg_xml_state_characters,
    rsvg_xml_state_end_element,
    rsvg_xml_state_entity_insert,
    rsvg_xml_state_entity_lookup,
    rsvg_xml_state_error,
    rsvg_xml_state_free,
    rsvg_xml_state_new,
    rsvg_xml_state_start_element,
    rsvg_xml_state_steal_result,
};

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
mod create_node;
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
mod io;
mod iri;
mod length;
mod link;
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
mod style;
pub mod surface_utils;
mod text;
mod transform;
mod tree;
mod unitinterval;
mod util;
mod viewbox;
mod viewport;
mod xml;
