#![allow(clippy::needless_doctest_main)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_logo_url = "https://gnome.pages.gitlab.gnome.org/librsvg/Rsvg-2.0/librsvg-r.svg")]
//! # Rust librsvg bindings
//!
//! This library contains safe Rust bindings for librsvg

/// No-op.
macro_rules! skip_assert_initialized {
    () => {};
}

// Re-export the -sys bindings
pub use ffi;
pub use gio;
pub use glib;

/// No-op.
macro_rules! assert_initialized_main_thread {
    () => {};
}

mod auto;
mod handle;
mod length;
mod rectangle;
mod unit;

pub use auto::*;
pub use length::*;
pub use rectangle::*;
pub mod prelude;
