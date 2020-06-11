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
//! document.  It holds the tree of [`Node`] elements, and a mapping of `id` attributes
//! to the corresponding element nodes.
//!
//! * [The `node` module](node/index.html) provides the [`Node`] struct and
//! helper traits used to operate on nodes.
//!
//! * [The `element` module](element/index.html) provides the [`Element`] struct and
//! the [`SetAttributes`] and [`Draw`] traits which are implemented by all SVG elements.
//!
//! * [The `xml` module](xml/index.html) receives events from the XML parser, and builds a
//! [`Document`] as a tree of [`Node`].
//!
//! * [The `properties` module](properties/index.html) contains structs that represent
//! collections of CSS properties.
//!
//! * [The `property_defs` module](property_defs/index.html) contains one type for each of
//! the CSS style properties that librsvg supports.
//!
//! * [The `css` module](css/index.html) contains the implementation of CSS parsing
//! and matching.
//!
//! [`Document`]: document/struct.Document.html
//! [`Node`]: node/type.Node.html
//! [`Element`]: element/struct.Element.html
//! [`SetAttributes`]: element/trait.SetAttributes.html
//! [`Draw`]: element/trait.Draw.html

#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![warn(unused)]

pub use crate::color::Color;

pub use crate::dpi::Dpi;

pub use crate::error::{DefsLookupErrorKind, HrefError, LoadingError, RenderingError};

pub use crate::handle::{Handle, LoadOptions};

pub use crate::length::{Length, LengthUnit, RsvgLength};

pub use crate::parsers::Parse;

pub use crate::rect::{IRect, Rect};

pub use crate::structure::IntrinsicDimensions;

pub use crate::surface_utils::{
    iterators::Pixels,
    shared_surface::{SharedImageSurface, SurfaceType},
    CairoARGB, Pixel,
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
mod node;

#[macro_use]
mod property_macros;

mod allowed_url;
mod angle;
mod aspect_ratio;
mod bbox;
mod color;
mod cond;
mod css;
mod dasharray;
mod document;
mod dpi;
mod drawing_ctx;
mod element;
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
mod marker;
mod number_list;
mod paint_server;
pub mod path_builder; // pub for benchmarking
pub mod path_parser; // pub for benchmarking
mod pattern;
mod properties;
mod property_bag;
mod property_defs;
pub mod rect;
mod shapes;
mod space;
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
