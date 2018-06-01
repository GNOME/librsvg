//! Utility functions for dealing with sRGB colors.
//!
//! The constant values in this module are taken from http://www.color.org/chardata/rgb/srgb.xalter
use std::slice;

use cairo;
use cairo::prelude::SurfaceExt;
use cairo_sys;

use filters::context::IRect;

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

/// Converts an sRGB surface to a linear sRGB surface (undoes the gamma correction).
///
/// The returned surface is transparent everywhere except the rectangle defined by `bounds`.
pub fn linearize_surface(
    surface: &cairo::ImageSurface,
    bounds: IRect,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let width = surface.get_width();
    let height = surface.get_height();
    let input_stride = surface.get_stride();

    assert!(bounds.x0 >= 0);
    assert!(bounds.y0 >= 0);
    assert!(bounds.x1 < width);
    assert!(bounds.y1 < height);

    // TODO: this currently gives "non-exclusive access" (can we make read-only borrows?)
    // let input_data = surface.get_data().unwrap();
    surface.flush();
    if surface.status() != cairo::Status::Success {
        return Err(surface.status());
    }
    let input_data_ptr = unsafe { cairo_sys::cairo_image_surface_get_data(surface.to_raw_none()) };
    if input_data_ptr.is_null() {
        return Err(cairo::Status::SurfaceFinished);
    }
    let input_data_len = input_stride as usize * height as usize;
    let input_data = unsafe { slice::from_raw_parts(input_data_ptr, input_data_len) };

    let mut output_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let output_stride = output_surface.get_stride();

    {
        let mut output_data = output_surface.get_data().unwrap();

        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                let input_index = (y * input_stride + x * 4) as usize;
                let output_index = (y * output_stride + x * 4) as usize;

                let alpha = input_data[input_index + 3];

                if alpha > 0 {
                    let alpha = f64::from(alpha) / 255f64;

                    for c in 0..3 {
                        let input_value = f64::from(input_data[input_index + c]) / 255f64;
                        let input_value = input_value / alpha; // Unpremultiply alpha.

                        let output_value = linearize(input_value);
                        let output_value = output_value * alpha; // Premultiply alpha again.

                        output_data[output_index + c] = (output_value * 255f64).round() as u8;
                    }
                }

                output_data[output_index + 3] = alpha;
            }
        }
    }

    Ok(output_surface)
}

/// Converts a linear sRGB surface to a normal sRGB surface (applies the gamma correction).
///
/// The returned surface is transparent everywhere except the rectangle defined by `bounds`.
pub fn unlinearize_surface(
    surface: &cairo::ImageSurface,
    bounds: IRect,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let width = surface.get_width();
    let height = surface.get_height();
    let input_stride = surface.get_stride();

    assert!(bounds.x0 >= 0);
    assert!(bounds.y0 >= 0);
    assert!(bounds.x1 < width);
    assert!(bounds.y1 < height);

    // TODO: this currently gives "non-exclusive access" (can we make read-only borrows?)
    // let input_data = surface.get_data().unwrap();
    surface.flush();
    if surface.status() != cairo::Status::Success {
        return Err(surface.status());
    }
    let input_data_ptr = unsafe { cairo_sys::cairo_image_surface_get_data(surface.to_raw_none()) };
    if input_data_ptr.is_null() {
        return Err(cairo::Status::SurfaceFinished);
    }
    let input_data_len = input_stride as usize * height as usize;
    let input_data = unsafe { slice::from_raw_parts(input_data_ptr, input_data_len) };

    let mut output_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let output_stride = output_surface.get_stride();

    {
        let mut output_data = output_surface.get_data().unwrap();

        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                let input_index = (y * input_stride + x * 4) as usize;
                let output_index = (y * output_stride + x * 4) as usize;

                let alpha = input_data[input_index + 3];

                if alpha > 0 {
                    let alpha = f64::from(alpha) / 255f64;

                    for c in 0..3 {
                        let input_value = f64::from(input_data[input_index + c]) / 255f64;
                        let input_value = input_value / alpha; // Unpremultiply alpha.

                        let output_value = unlinearize(input_value);
                        let output_value = output_value * alpha; // Premultiply alpha again.

                        output_data[output_index + c] = (output_value * 255f64).round() as u8;
                    }
                }

                output_data[output_index + 3] = alpha;
            }
        }
    }

    Ok(output_surface)
}
