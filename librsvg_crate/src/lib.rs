#![warn(unused)]
extern crate rsvg_internals;
extern crate url;

use url::Url;

/// Full configuration for loading an [`SvgHandle`][SvgHandle]
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
}

pub struct SvgHandle {
}
