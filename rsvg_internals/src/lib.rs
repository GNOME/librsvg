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
extern crate xml as xml_rs;

#[macro_use]
extern crate lazy_static;

pub use color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use drawing_ctx::{
    rsvg_drawing_ctx_add_node_and_ancestors_to_stack,
    rsvg_drawing_ctx_draw_node_from_stack,
    rsvg_drawing_ctx_free,
    rsvg_drawing_ctx_get_geometry,
    rsvg_drawing_ctx_new,
};

pub use handle::{
    rsvg_handle_acquire_data,
    rsvg_handle_acquire_stream,
    rsvg_handle_defs_lookup,
    rsvg_handle_rust_cascade,
    rsvg_handle_rust_free,
    rsvg_handle_rust_get_base_gfile,
    rsvg_handle_rust_get_dpi_x,
    rsvg_handle_rust_get_dpi_y,
    rsvg_handle_rust_get_root,
    rsvg_handle_rust_new,
    rsvg_handle_rust_node_is_root,
    rsvg_handle_rust_set_base_url,
    rsvg_handle_rust_set_dpi_x,
    rsvg_handle_rust_set_dpi_y,
    rsvg_handle_rust_steal_result,
};

pub use io::rsvg_get_input_stream_for_loading;

pub use node::rsvg_node_unref;

pub use structure::rsvg_node_svg_get_size;

pub use xml::{
    rsvg_xml_state_error,
    rsvg_xml_state_free,
    rsvg_xml_state_new,
    rsvg_xml_state_tree_is_valid,
};

pub use xml2_load::{
    rsvg_create_xml_push_parser,
    rsvg_set_error_from_xml,
    rsvg_xml_state_parse_from_stream,
};

#[macro_use]
mod log;

#[macro_use]
mod coord_units;

#[macro_use]
mod float_eq_cairo;

#[macro_use]
mod property_macros;

mod allowed_url;
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
mod dpi;
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
mod svg;
mod text;
mod transform;
mod tree;
pub mod tree_utils;
mod unitinterval;
mod util;
mod viewbox;
mod viewport;
mod xml;
mod xml2;
mod xml2_load;
