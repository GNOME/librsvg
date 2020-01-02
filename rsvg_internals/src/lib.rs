//! # The implementation of librsvg
//!
//! The implementation of librsvg is in the `rsvg_internals` crate.  It is not a public
//! crate; instead, it exports the primitives necessary to implement librsvg's public APIs,
//! both the C and Rust APIs.  It has the XML and CSS parsing code, the SVG element
//! definitions and tree of elements, and all the drawing logic.
//!
//! Some interesting parts of rsvg_internals:
//!
//! * [The `Handle` struct](handle/struct.Handle.html) provides the primitives to implement
//! the public APIs, such as loading an SVG file and rendering it.
//!
//! * [The `DrawingCtx` struct](drawing_ctx/struct.DrawingCtx.html) is active while an SVG
//! handle is being drawn or queried for its geometry.  It has all the mutable state related
//! to the drawing process.
//!
//! * [The `Document` struct](document/struct.Document.html) represents a loaded SVG
//! document.  It holds the tree of [`RsvgNode`] elements, and a mapping of `id` attributes
//! to the corresponding element nodes.
//!
//! * [The `node` module](node/index.html) provides the [`RsvgNode`] struct and
//! [`NodeTrait`], which form the basis for the tree of SVG elements.
//!
//! * [The `xml` module](xml/index.html) receives events from the XML parser, and builds a
//! [`Document`] as a tree of [`RsvgNode`].
//!
//! * [The `properties` module](properties/index.html) contains structs that represent
//! collections of CSS properties.
//!
//! * [The `property_defs` module](property_defs/index.html) contains one type for each of
//! the CSS style properties that librsvg supports.
//!
//! [`Document`]: document/struct.Document.html
//! [`RsvgNode`]: node/type.RsvgNode.html
//! [`NodeTrait`]: node/trait.NodeTrait.html

#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![warn(unused)]
use ::xml as xml_rs;

pub use crate::color::{rsvg_css_parse_color, ColorKind, ColorSpec};

pub use crate::dpi::{rsvg_rust_set_default_dpi_x_y, Dpi};

pub use crate::error::{DefsLookupErrorKind, HrefError, LoadingError, RenderingError};

pub use crate::handle::{
    Handle,
    LoadOptions,
    RsvgDimensionData,
    RsvgPositionData,
    RsvgSizeFunc,
    SizeCallback,
};

pub use crate::length::{Length, LengthUnit, RsvgLength};

pub use crate::rect::IRect;

pub use crate::structure::IntrinsicDimensions;

pub use crate::surface_utils::{
    iterators::Pixels,
    shared_surface::{SharedImageSurface, SurfaceType},
};

pub use crate::viewbox::ViewBox;

#[macro_use]
pub mod log;

#[macro_use]
mod parsers;

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
mod color;
mod cond;
mod create_node;
mod css;
mod dasharray;
mod document;
mod dpi;
mod drawing_ctx;
mod error;
mod filter;
pub mod filters;
mod font_props;
mod gradient;
mod handle;
mod image;
mod io;
mod iri;
mod length;
mod limits;
mod link;
mod marker;
mod node;
mod number_list;
mod paint_server;
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
mod text;
mod transform;
mod unit_interval;
mod util;
mod viewbox;
mod xml;
mod xml2;
mod xml2_load;
