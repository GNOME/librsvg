//! Utility functions for dealing with sRGB colors.
//!
//! The constant values in this module are taken from http://www.color.org/chardata/rgb/srgb.xalter
use cairo;

use filters::context::IRect;
use surface_utils::{
    iterators::Pixels,
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt,
    Pixel,
};

// Include the linearization and unlinearization tables.
include!(concat!(env!("OUT_DIR"), "/srgb-codegen.rs"));

/// Converts an sRGB color value to a linear sRGB color value (undoes the gamma correction).
#[inline]
pub fn linearize(c: u8) -> u8 {
    LINEARIZE[usize::from(c)]
}

/// Converts a linear sRGB color value to a normal sRGB color value (applies the gamma correction).
#[inline]
pub fn unlinearize(c: u8) -> u8 {
    UNLINEARIZE[usize::from(c)]
}

/// Processing loop of `map_unpremultiplied_components`. Extracted (and public) for benchmarking.
#[inline]
pub fn map_unpremultiplied_components_loop<F: Fn(u8) -> u8>(
    surface: &SharedImageSurface,
    output_surface: &mut cairo::ImageSurface,
    bounds: IRect,
    f: F,
) {
    let output_stride = output_surface.get_stride() as usize;
    {
        let mut output_data = output_surface.get_data().unwrap();

        for (x, y, pixel) in Pixels::new(surface, bounds) {
            if pixel.a > 0 {
                let alpha = f64::from(pixel.a) / 255f64;

                let compute = |x| {
                    let x = f64::from(x) / alpha; // Unpremultiply alpha.
                    let x = (x + 0.5) as u8; // Round to nearest u8.
                    let x = f(x);
                    let x = f64::from(x) * alpha; // Premultiply alpha again.
                    (x + 0.5) as u8
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
}

/// Applies the function to each pixel component after unpremultiplying.
fn map_unpremultiplied_components<F: Fn(u8) -> u8>(
    surface: &SharedImageSurface,
    bounds: IRect,
    f: F,
    new_type: SurfaceType,
) -> Result<SharedImageSurface, cairo::Status> {
    // This function doesn't affect the alpha channel.
    if surface.is_alpha_only() {
        return Ok(surface.clone());
    }

    let width = surface.width();
    let height = surface.height();

    let mut output_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    map_unpremultiplied_components_loop(surface, &mut output_surface, bounds, f);

    SharedImageSurface::new(output_surface, new_type)
}

/// Converts an sRGB surface to a linear sRGB surface (undoes the gamma correction).
#[inline]
pub fn linearize_surface(
    surface: &SharedImageSurface,
    bounds: IRect,
) -> Result<SharedImageSurface, cairo::Status> {
    assert_ne!(surface.surface_type(), SurfaceType::LinearRgb);

    map_unpremultiplied_components(surface, bounds, linearize, SurfaceType::LinearRgb)
}

/// Converts a linear sRGB surface to a normal sRGB surface (applies the gamma correction).
#[inline]
pub fn unlinearize_surface(
    surface: &SharedImageSurface,
    bounds: IRect,
) -> Result<SharedImageSurface, cairo::Status> {
    assert_ne!(surface.surface_type(), SurfaceType::SRgb);

    map_unpremultiplied_components(surface, bounds, unlinearize, SurfaceType::SRgb)
}
