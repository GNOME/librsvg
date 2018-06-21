//! Various utilities for working with Cairo image surfaces.
use std::ops::DerefMut;

use cairo;

pub mod iterators;
pub mod shared_surface;

/// A pixel consisting of R, G, B and A values.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Extension methods for `cairo::ImageSurfaceData`.
pub trait ImageSurfaceDataExt: DerefMut<Target = [u8]> {
    /// Sets the pixel at the given coordinates.
    #[inline]
    fn set_pixel(&mut self, stride: usize, pixel: Pixel, x: u32, y: u32) {
        let value = ((pixel.a as u32) << 24)
            | ((pixel.r as u32) << 16)
            | ((pixel.g as u32) << 8)
            | (pixel.b as u32);
        unsafe {
            *(&mut self[y as usize * stride + x as usize * 4] as *mut u8 as *mut u32) = value;
        }
    }
}

impl<'a> ImageSurfaceDataExt for cairo::ImageSurfaceData<'a> {}
