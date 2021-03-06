//! The implementation of librsvg.
//!
//! The implementation of librsvg is in the `rsvg_internals` crate.  It is not a public
//! crate; instead, it exports the primitives necessary to implement librsvg's public APIs,
//! both the C and Rust APIs.  It has the XML and CSS parsing code, the SVG element
//! definitions and tree of elements, and all the drawing logic.
//!
//! # Common entry points for newcomers
//!
//! * Are you adding support for a CSS property?  Look in the [`property_defs`] module.
//!
//! # Some interesting parts of rsvg_internals
//!
//! * The [`Handle`] struct provides the primitives to implement the public APIs, such as
//! loading an SVG file and rendering it.
//!
//! * The [`DrawingCtx`] struct is active while an SVG handle is being drawn or queried
//! for its geometry.  It has all the mutable state related to the drawing process.
//!
//! * The [`Document`] struct represents a loaded SVG document.  It holds the tree of
//! [`Node`] elements, and a mapping of `id` attributes to the corresponding element
//! nodes.
//!
//! * The [`node`] module provides the [`Node`] struct and helper traits used to operate
//! on nodes.
//!
//! * The [`element`] module provides the [`Element`] struct and the [`SetAttributes`] and
//! [`Draw`] traits which are implemented by all SVG elements.
//!
//! * The [`xml`] module receives events from the XML parser, and builds a [`Document`] as
//! a tree of [`Node`].
//!
//! * The [`properties`] module contains structs that represent collections of CSS
//! properties.
//!
//! * The [`property_defs`] module contains one type for each of the CSS style properties
//! that librsvg supports.
//!
//! * The [`css`] module contains the implementation of CSS parsing and matching.
//!
//! [`Document`]: document/struct.Document.html
//! [`Node`]: node/type.Node.html
//! [`Element`]: element/struct.Element.html
//! [`Handle`]: handle/struct.Handle.html
//! [`DrawingCtx`]: drawing_ctx/struct.DrawingCtx.html
//! [`Document`]: document/struct.Document.html
//! [`SetAttributes`]: element/trait.SetAttributes.html
//! [`Draw`]: element/trait.Draw.html
//! [`css`]: css/index.html
//! [`element`]: element/index.html
//! [`node`]: node/index.html
//! [`properties`]: properties/index.html
//! [`property_defs`]: property_defs/index.html
//! [`xml`]: xml/index.html

#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![warn(nonstandard_style, rust_2018_idioms, unused)]
// Some lints no longer exist
#![warn(renamed_and_removed_lints)]
// Standalone lints
#![warn(trivial_casts, trivial_numeric_casts)]

// The public API is exported here
pub use crate::api::*;

pub use crate::color::Color;

pub use crate::parsers::Parse;

pub use crate::rect::{IRect, Rect};

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

#[macro_use]
mod util;

mod angle;
mod api;
mod aspect_ratio;
mod bbox;
pub mod c_api;
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
mod href;
mod image;
mod io;
mod iri;
mod length;
mod limits;
mod marker;
mod paint_server;
mod path_builder;
mod path_parser;
mod pattern;
mod properties;
mod property_defs;
mod rect;
mod shapes;
mod space;
mod structure;
mod style;
pub mod surface_utils;
mod text;
mod transform;
mod unit_interval;
mod url_resolver;
mod viewbox;
mod xml;

#[doc(hidden)]
pub mod bench_only {
    pub use crate::path_builder::PathBuilder;
    pub use crate::path_parser::{parse_path_into_builder, Lexer};
}

#[doc(hidden)]
pub mod doctest_only {
    pub use crate::aspect_ratio::AspectRatio;
    pub use crate::error::AttributeResultExt;
    pub use crate::error::ElementError;
    pub use crate::error::ValueErrorKind;
    pub use crate::href::is_href;
    pub use crate::href::set_href;
    pub use crate::length::{Both, CssLength, Horizontal, Length, LengthUnit, ULength, Vertical};
    pub use crate::parsers::{Parse, ParseValue};
}

#[doc(hidden)]
pub mod rsvg_convert_only {
    pub use crate::c_api::handle::PathOrUrl;
    pub use crate::c_api::sizing::LegacySize;
}
