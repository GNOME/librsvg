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

/// Modes which specify how the values of out of bounds pixels are computed.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EdgeMode {
    /// The nearest inbounds pixel value is returned.
    Duplicate,
    /// The image is extended by taking the color values from the opposite of the image.
    ///
    /// Imagine the image being tiled infinitely, with the original image at the origin.
    Wrap,
    /// Zero RGBA values are returned.
    None,
}

/// Extension methods for `cairo::ImageSurfaceData`.
pub trait ImageSurfaceDataExt: DerefMut<Target = [u8]> {
    /// Sets the pixel at the given coordinates. Assumes the `ARgb32` format.
    #[inline]
    fn set_pixel(&mut self, stride: usize, pixel: Pixel, x: u32, y: u32) {
        let value = pixel.to_u32();
        unsafe {
            *(&mut self[y as usize * stride + x as usize * 4] as *mut u8 as *mut u32) = value;
        }
    }
}

impl Pixel {
    /// Returns an unpremultiplied value of this pixel.
    #[inline]
    pub fn unpremultiply(self) -> Self {
        if self.a == 0 {
            self
        } else {
            let alpha = f64::from(self.a) / 255.0;
            let unpremultiply = |x| ((f64::from(x) / alpha) + 0.5) as u8;

            Self {
                r: unpremultiply(self.r),
                g: unpremultiply(self.g),
                b: unpremultiply(self.b),
                a: self.a,
            }
        }
    }

    /// Returns a premultiplied value of this pixel.
    #[inline]
    pub fn premultiply(self) -> Self {
        let alpha = f64::from(self.a) / 255.0;
        let premultiply = |x| ((f64::from(x) * alpha) + 0.5) as u8;

        Self {
            r: premultiply(self.r),
            g: premultiply(self.g),
            b: premultiply(self.b),
            a: self.a,
        }
    }

    /// Returns the pixel value as a `u32`, in the same format as `cairo::Format::ARgb32`.
    #[inline]
    pub fn to_u32(self) -> u32 {
        (u32::from(self.a) << 24)
            | (u32::from(self.r) << 16)
            | (u32::from(self.g) << 8)
            | u32::from(self.b)
    }

    /// Converts a `u32` in the same format as `cairo::Format::ARgb32` into a `Pixel`.
    #[inline]
    pub fn from_u32(x: u32) -> Self {
        Self {
            r: ((x >> 16) & 0xFF) as u8,
            g: ((x >> 8) & 0xFF) as u8,
            b: (x & 0xFF) as u8,
            a: ((x >> 24) & 0xFF) as u8,
        }
    }
}

impl<'a> ImageSurfaceDataExt for cairo::ImageSurfaceData<'a> {}
impl<'a> ImageSurfaceDataExt for &'a mut [u8] {}
