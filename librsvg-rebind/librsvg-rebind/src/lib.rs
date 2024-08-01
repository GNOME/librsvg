#![allow(clippy::needless_doctest_main)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_logo_url = "https://gnome.pages.gitlab.gnome.org/librsvg/Rsvg-2.0/librsvg-r.svg")]
//! # Rust librsvg bindings
//!
//! This package contains safe Rust bindings for the librsvg C library.
//!
//! Since librsvg is written in Rust, the native [`rsvg`] crate is available
//! to use the same features. One of the main purposes of *librsvg-rebind*
//! is to reduce the binary sice.
//!
//! To use this package, the *librsvg-2* library has to be available on the system.
//! If you use the native [`rsvg`] crate, this is not required.
//!
//! [`rsvg`]: https://crates.io/crates/librsvg
//!
//! # Example
//!
//! ```
//! use librsvg_rebind::prelude::*;
//!
//! let handle = librsvg_rebind::Handle::from_file("../../rsvg/example.svg")
//!     .unwrap()
//!     .unwrap();
//!
//! let (width, height) = handle.intrinsic_size_in_pixels().unwrap();
//!
//! let surface =
//!     cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32).unwrap();
//! let context = cairo::Context::new(&surface).unwrap();
//!
//! let viewport = librsvg_rebind::Rectangle::new(0., 0., height, width);
//!
//! handle.render_document(&context, &viewport).unwrap();
//!
//! let mut output_file = std::fs::File::create("/dev/null").unwrap();
//! surface.write_to_png(&mut output_file).unwrap();
//! ```

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
