//! Utility functions for dealing with sRGB colors.
//!
//! The constant values in this module are taken from http://www.color.org/chardata/rgb/srgb.xalter
use cairo;

use filters::context::IRect;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    ImageSurfaceDataExt,
    Pixel,
};

/// Converts an sRGB color value to a linear sRGB color value (undoes the gamma correction).
///
/// The input and the output are supposed to be in the [0, 1] range.
#[inline]
pub fn linearize(c: f64) -> f64 {
    if c <= (12.92 * 0.0031308) {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a linear sRGB color value to a normal sRGB color value (applies the gamma correction).
///
/// The input and the output are supposed to be in the [0, 1] range.
#[inline]
pub fn unlinearize(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1f64 / 2.4) - 0.055
    }
}

/// Applies the function to each pixel component after unpremultiplying.
///
/// The returned surface is transparent everywhere except the rectangle defined by `bounds`.
fn map_unpremultiplied_components<F>(
    surface: &SharedImageSurface,
    bounds: IRect,
    f: F,
) -> Result<cairo::ImageSurface, cairo::Status>
where
    F: Fn(f64) -> f64,
{
    let width = surface.width();
    let height = surface.height();

    let mut output_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let output_stride = output_surface.get_stride() as usize;
    {
        let mut output_data = output_surface.get_data().unwrap();

        for (x, y, pixel) in Pixels::new(surface, bounds) {
            if pixel.a > 0 {
                let alpha = f64::from(pixel.a) / 255f64;

                let compute = |x| {
                    let x = f64::from(x) / 255f64;
                    let x = x / alpha; // Unpremultiply alpha.
                    let x = f(x);
                    let x = x * alpha; // Premultiply alpha again.
                    (x * 255f64).round() as u8
                };

                let output_pixel = Pixel {
                    r: compute(pixel.r),
                    g: compute(pixel.g),
                    b: compute(pixel.b),
                    a: pixel.a,
                };
                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }
    }

    Ok(output_surface)
}

/// Converts an sRGB surface to a linear sRGB surface (undoes the gamma correction).
///
/// The returned surface is transparent everywhere except the rectangle defined by `bounds`.
#[inline]
pub fn linearize_surface(
    surface: &SharedImageSurface,
    bounds: IRect,
) -> Result<cairo::ImageSurface, cairo::Status> {
    map_unpremultiplied_components(surface, bounds, linearize)
}

/// Converts a linear sRGB surface to a normal sRGB surface (applies the gamma correction).
///
/// The returned surface is transparent everywhere except the rectangle defined by `bounds`.
#[inline]
pub fn unlinearize_surface(
    surface: &SharedImageSurface,
    bounds: IRect,
) -> Result<cairo::ImageSurface, cairo::Status> {
    map_unpremultiplied_components(surface, bounds, unlinearize)
}
