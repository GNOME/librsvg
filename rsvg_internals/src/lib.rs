#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![warn(unused)]

use ::xml as xml_rs;

pub use crate::color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use crate::dpi::{rsvg_rust_set_default_dpi_x_y, Dpi};

pub use crate::drawing_ctx::RsvgRectangle;

pub use crate::error::{
    rsvg_rust_error_quark, set_gerror, DefsLookupErrorKind, HrefError, LoadingError,
    RenderingError, RSVG_ERROR_FAILED,
};

pub use crate::handle::{
    Handle, LoadOptions, RsvgDimensionData, RsvgPositionData, RsvgSizeFunc, SizeCallback,
};

pub use crate::length::{Length, LengthUnit, RsvgLength};

pub use crate::rect::IRect;

pub use crate::structure::IntrinsicDimensions;

pub use crate::surface_utils::{
    iterators::Pixels,
    shared_surface::{
        SharedImageSurface, SurfaceType,
    },
};

#[macro_use]
pub mod log;

#[macro_use]
mod coord_units;

#[macro_use]
mod float_eq_cairo;

#[macro_use]
mod property_macros;

mod allowed_url;
mod angle;
mod aspect_ratio;
mod bbox;
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
mod properties;
mod property_bag;
mod property_defs;
pub mod rect;
mod shapes;
mod space;
pub mod srgb;
mod structure;
mod style;
pub mod surface_utils;
mod svg;
mod text;
mod transform;
mod unit_interval;
mod util;
mod viewbox;
mod xml;
mod xml2;
mod xml2_load;
