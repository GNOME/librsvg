#![cfg_attr(feature = "cargo-clippy", allow(clone_on_ref_ptr))]
#![cfg_attr(feature = "cargo-clippy", allow(not_unsafe_ptr_arg_deref))]
#![cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]
#![warn(unused)]

extern crate cairo;
extern crate cairo_sys;
extern crate cssparser;
extern crate data_url;
extern crate downcast_rs;
extern crate encoding;
extern crate float_cmp;
extern crate gdk_pixbuf;
extern crate gdk_pixbuf_sys;
extern crate gio;
extern crate glib_sys;
extern crate gobject_sys;
extern crate itertools;
extern crate language_tags;
extern crate libc;
extern crate locale_config;
extern crate nalgebra;
extern crate num_traits;
extern crate owning_ref;
extern crate pango;
extern crate pango_sys;
extern crate pangocairo;
extern crate rayon;
extern crate regex;
extern crate url;
extern crate xml as xml_rs;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate glib;

#[macro_use]
extern crate lazy_static;

pub use c_api::{
    rsvg_rust_error_get_type,
    rsvg_rust_handle_close,
    rsvg_rust_handle_flags_get_type,
    rsvg_rust_handle_get_base_url,
    rsvg_rust_handle_get_dimensions,
    rsvg_rust_handle_get_dimensions_sub,
    rsvg_rust_handle_get_dpi_x,
    rsvg_rust_handle_get_dpi_y,
    rsvg_rust_handle_get_flags,
    rsvg_rust_handle_get_geometry_sub,
    rsvg_rust_handle_get_intrinsic_dimensions,
    rsvg_rust_handle_get_pixbuf_sub,
    rsvg_rust_handle_get_position_sub,
    rsvg_rust_handle_has_sub,
    rsvg_rust_handle_new_from_data,
    rsvg_rust_handle_new_from_file,
    rsvg_rust_handle_new_from_gfile_sync,
    rsvg_rust_handle_new_from_stream_sync,
    rsvg_rust_handle_new_with_flags,
    rsvg_rust_handle_read_stream_sync,
    rsvg_rust_handle_render_cairo_sub,
    rsvg_rust_handle_set_base_gfile,
    rsvg_rust_handle_set_base_url,
    rsvg_rust_handle_set_dpi_x,
    rsvg_rust_handle_set_dpi_y,
    rsvg_rust_handle_set_flags,
    rsvg_rust_handle_set_size_callback,
    rsvg_rust_handle_set_testing,
    rsvg_rust_handle_write,
};

pub use color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use dpi::{rsvg_rust_set_default_dpi_x_y, Dpi};

pub use drawing_ctx::RsvgRectangle;

pub use error::{
    rsvg_rust_error_quark,
    DefsLookupErrorKind,
    HrefError,
    LoadingError,
    RenderingError,
};

pub use handle::{Handle, LoadFlags};

pub use length::{Length, LengthUnit};

pub use pixbuf_utils::{
    rsvg_rust_pixbuf_from_file_at_max_size,
    rsvg_rust_pixbuf_from_file_at_size,
    rsvg_rust_pixbuf_from_file_at_zoom,
    rsvg_rust_pixbuf_from_file_at_zoom_with_max,
};

pub use rect::IRect;

#[macro_use]
mod log;

#[macro_use]
mod coord_units;

#[macro_use]
mod float_eq_cairo;

#[macro_use]
mod property_macros;

mod allowed_url;
mod angle;
mod aspect_ratio;
mod attributes;
mod bbox;
mod c_api;
mod clip_path;
mod color;
mod cond;
mod create_node;
mod croco;
mod css;
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
mod number_list;
mod paint_server;
mod parsers;
mod path_builder;
mod path_parser;
mod pattern;
mod pixbuf_utils;
mod properties;
mod property_bag;
mod rect;
mod shapes;
mod space;
pub mod srgb;
mod stop;
mod structure;
mod style;
pub mod surface_utils;
mod svg;
mod text;
mod transform;
pub mod tree_utils;
mod unit_interval;
mod util;
mod viewbox;
mod viewport;
mod xml;
mod xml2;
mod xml2_load;
