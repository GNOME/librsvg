//! Various utilities for working with Cairo image surfaces.
use std::ops::DerefMut;

use cairo;
use cairo_sys;
use glib::translate::*;

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

    /// Returns a 'mask' pixel with only the alpha channel
    ///
    /// Assuming, the pixel is linear RGB (not sRGB)
    /// y = luminance
    /// Y = 0.2126 R + 0.7152 G + 0.0722 B
    /// 1.0 opacity = 255
    ///
    /// When Y = 1.0, pixel for mask should be 0xFFFFFFFF
    /// (you get 1.0 luminance from 255 from R, G and B)
    ///
    /// r_mult = 0xFFFFFFFF / (255.0 * 255.0) * .2126 = 14042.45  ~= 14042
    /// g_mult = 0xFFFFFFFF / (255.0 * 255.0) * .7152 = 47239.69  ~= 47240
    /// b_mult = 0xFFFFFFFF / (255.0 * 255.0) * .0722 =  4768.88  ~= 4769
    ///
    /// This allows for the following expected behaviour:
    ///    (we only care about the most sig byte)
    /// if pixel = 0x00FFFFFF, pixel' = 0xFF......
    /// if pixel = 0x00020202, pixel' = 0x02......
    /// if pixel = 0x00000000, pixel' = 0x00......
    pub fn to_mask(self, opacity: u8) -> Self {
        let r = self.r as u32;
        let g = self.g as u32;
        let b = self.b as u32;
        let o = opacity as u32;

        Self {
            r: 0,
            g: 0,
            b: 0,
            a: (((r * 14042 + g * 47240 + b * 4769) * o) >> 24) as u8,
        }
    }

    #[inline]
    pub fn diff(self, p: &Pixel) -> Pixel {
        let a_r = self.r as i32;
        let a_g = self.g as i32;
        let a_b = self.b as i32;
        let a_a = self.a as i32;

        let b_r = p.r as i32;
        let b_g = p.g as i32;
        let b_b = p.b as i32;
        let b_a = p.a as i32;

        let r = (a_r - b_r).abs() as u8;
        let g = (a_g - b_g).abs() as u8;
        let b = (a_b - b_b).abs() as u8;
        let a = (a_a - b_a).abs() as u8;

        Pixel { r, g, b, a }
    }
}

impl<'a> ImageSurfaceDataExt for cairo::ImageSurfaceData<'a> {}
impl<'a> ImageSurfaceDataExt for &'a mut [u8] {}

// FIXME: cairo-rs forgot to export its own SurfaceExt with the status() method
// and others: https://github.com/gtk-rs/cairo/issues/252
//
// Remove the following when cairo-rs gets fixed.
pub trait SurfaceExt {
    fn status(&self) -> cairo::Status;
}

impl SurfaceExt for cairo::Surface {
    fn status(&self) -> cairo::Status {
        unsafe {
            let raw_surface = self.to_glib_none();

            cairo_sys::cairo_surface_status(raw_surface.0).into()
        }
    }
}
