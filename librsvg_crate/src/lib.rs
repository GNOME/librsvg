#![warn(unused)]
extern crate gio;
extern crate glib;
extern crate rsvg_internals;
extern crate url;

use std::path::Path;

use gio::FileExt;
use glib::object::Cast;

use rsvg_internals::{Handle, LoadFlags};
use url::Url;

pub use rsvg_internals::{LoadingError, RenderingError};

/// Full configuration for loading an [`SvgHandle`][SvgHandle]
///
/// This struct implements a builder pattern for configuring an
/// [`SvgHandle`][SvgHandle]'s options, and then loading the SVG data.
/// You can call the methods of `LoadOptions` in sequence to configure
/// how SVG data should be loaded, and finally use one of the loading
/// functions to load an [`SvgHandle`][SvgHandle].
///
/// # Example:
///
/// ```ignore
/// extern crate librsvg;
///
/// use librsvg::LoadOptions;
///
/// let svg_handle = LoadOptions::new()
///     .unlimited_size()
///     .read_path("example.svg")
///     .unwrap();
/// ```
///
/// [SvgHandle]: struct.SvgHandle.html
pub struct LoadOptions {
    unlimited_size: bool,
    keep_image_data: bool,
    base_url: Option<Url>,
}

impl LoadOptions {
    pub fn new() -> Self {
        LoadOptions {
            unlimited_size: false,
            keep_image_data: false,
            base_url: None,
        }
    }

    pub fn base_url(mut self, url: Option<&Url>) -> Self {
        self.base_url = url.map(|u| u.clone());
        self
    }

    pub fn unlimited_size(mut self, unlimited: bool) -> Self {
        self.unlimited_size = unlimited;
        self
    }

    pub fn keep_image_data(mut self, keep: bool) -> Self {
        self.keep_image_data = keep;
        self
    }

    fn load_flags(&self) -> LoadFlags {
        LoadFlags {
            unlimited_size: self.unlimited_size,
            keep_image_data: self.keep_image_data,
        }
    }

    pub fn read_path<P: AsRef<Path>>(self, path: P) -> Result<SvgHandle, LoadingError> {
        let file = gio::File::new_for_path(path);

        let stream = file.read(None)?;

        let mut handle = Handle::new_with_flags(self.load_flags());
        handle.construct_read_stream_sync(&stream.upcast(), Some(&file), None)?;

        Ok(SvgHandle(handle))
    }
}

pub struct SvgHandle(Handle);
