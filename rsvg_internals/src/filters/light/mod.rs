//! Light filters and nodes.
use nalgebra::Vector2;

use filters::context::IRect;
use surface_utils::shared_surface::SharedImageSurface;

pub mod light_source;
pub mod lighting;

// Functions here are pub for the purpose of accessing them from benchmarks.

/// 2D normal and factor stored separately.
///
/// The normal needs to be multiplied by `surface_scale * factor / 255` and normalized with 1 as
/// the z component.
#[derive(Debug, Clone, Copy)]
pub struct Normal {
    pub factor: Vector2<f64>,
    pub normal: Vector2<i16>,
}

/// Inner utility function.
#[inline]
fn return_normal(factor_x: f64, nx: i16, factor_y: f64, ny: i16) -> Normal {
    // Negative nx and ny to account for the different coordinate system.
    Normal {
        factor: Vector2::new(factor_x, factor_y),
        normal: Vector2::new(-nx, -ny),
    }
}

/// Computes and returns the normal vector for the top left pixel for light filters.
#[inline]
pub fn top_left_normal(surface: &SharedImageSurface, bounds: IRect) -> Normal {
    // Surface needs to be at least 2×2.
    assert!(bounds.x1 >= bounds.x0 + 2);
    assert!(bounds.y1 >= bounds.y0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let x = bounds.x0 as u32;
    let y = bounds.y0 as u32;

    let center = get(x, y);
    let right = get(x + 1, y);
    let bottom = get(x, y + 1);
    let bottom_right = get(x + 1, y + 1);

    return_normal(
        2. / 3.,
        -2 * center + 2 * right - bottom + bottom_right,
        2. / 3.,
        -2 * center - right + 2 * bottom + bottom_right,
    )
}

/// Computes and returns the normal vector for the top row pixels for light filters.
#[inline]
pub fn top_row_normal(surface: &SharedImageSurface, bounds: IRect, x: u32) -> Normal {
    assert!(x as i32 > bounds.x0);
    assert!((x as i32) + 1 < bounds.x1);
    assert!(bounds.y1 >= bounds.y0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let y = bounds.y0 as u32;

    let left = get(x - 1, y);
    let center = get(x, y);
    let right = get(x + 1, y);
    let bottom_left = get(x - 1, y + 1);
    let bottom = get(x, y + 1);
    let bottom_right = get(x + 1, y + 1);

    return_normal(
        1. / 3.,
        -2 * left + 2 * right - bottom_left + bottom_right,
        1. / 2.,
        -left - 2 * center - right + bottom_left + 2 * bottom + bottom_right,
    )
}

/// Computes and returns the normal vector for the top right pixel for light filters.
#[inline]
pub fn top_right_normal(surface: &SharedImageSurface, bounds: IRect) -> Normal {
    // Surface needs to be at least 2×2.
    assert!(bounds.x1 >= bounds.x0 + 2);
    assert!(bounds.y1 >= bounds.y0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let x = bounds.x1 as u32 - 1;
    let y = bounds.y0 as u32;

    let left = get(x - 1, y);
    let center = get(x, y);
    let bottom_left = get(x - 1, y + 1);
    let bottom = get(x, y + 1);

    return_normal(
        2. / 3.,
        -2 * left + 2 * center - bottom_left + bottom,
        2. / 3.,
        -left - 2 * center + bottom_left + 2 * bottom,
    )
}

/// Computes and returns the normal vector for the left column pixels for light filters.
#[inline]
pub fn left_column_normal(surface: &SharedImageSurface, bounds: IRect, y: u32) -> Normal {
    assert!(y as i32 > bounds.y0);
    assert!((y as i32) + 1 < bounds.y1);
    assert!(bounds.x1 >= bounds.x0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let x = bounds.x0 as u32;

    let top = get(x, y - 1);
    let top_right = get(x + 1, y - 1);
    let center = get(x, y);
    let right = get(x + 1, y);
    let bottom = get(x, y + 1);
    let bottom_right = get(x + 1, y + 1);

    return_normal(
        1. / 2.,
        -top + top_right - 2 * center + 2 * right - bottom + bottom_right,
        1. / 3.,
        -2 * top - top_right + 2 * bottom + bottom_right,
    )
}

/// Computes and returns the normal vector for the interior pixels for light filters.
#[inline]
pub fn interior_normal(surface: &SharedImageSurface, bounds: IRect, x: u32, y: u32) -> Normal {
    assert!(x as i32 > bounds.x0);
    assert!((x as i32) + 1 < bounds.x1);
    assert!(y as i32 > bounds.y0);
    assert!((y as i32) + 1 < bounds.y1);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);

    let top_left = get(x - 1, y - 1);
    let top = get(x, y - 1);
    let top_right = get(x + 1, y - 1);
    let left = get(x - 1, y);
    let right = get(x + 1, y);
    let bottom_left = get(x - 1, y + 1);
    let bottom = get(x, y + 1);
    let bottom_right = get(x + 1, y + 1);

    return_normal(
        1. / 4.,
        -top_left + top_right - 2 * left + 2 * right - bottom_left + bottom_right,
        1. / 4.,
        -top_left - 2 * top - top_right + bottom_left + 2 * bottom + bottom_right,
    )
}

/// Computes and returns the normal vector for the right column pixels for light filters.
#[inline]
pub fn right_column_normal(surface: &SharedImageSurface, bounds: IRect, y: u32) -> Normal {
    assert!(y as i32 > bounds.y0);
    assert!((y as i32) + 1 < bounds.y1);
    assert!(bounds.x1 >= bounds.x0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let x = bounds.x1 as u32 - 1;

    let top_left = get(x - 1, y - 1);
    let top = get(x, y - 1);
    let left = get(x - 1, y);
    let center = get(x, y);
    let bottom_left = get(x - 1, y + 1);
    let bottom = get(x, y + 1);

    return_normal(
        1. / 2.,
        -top_left + top - 2 * left + 2 * center - bottom_left + bottom,
        1. / 3.,
        -top_left - 2 * top + bottom_left + 2 * bottom,
    )
}

/// Computes and returns the normal vector for the bottom left pixel for light filters.
#[inline]
pub fn bottom_left_normal(surface: &SharedImageSurface, bounds: IRect) -> Normal {
    // Surface needs to be at least 2×2.
    assert!(bounds.x1 >= bounds.x0 + 2);
    assert!(bounds.y1 >= bounds.y0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let x = bounds.x0 as u32;
    let y = bounds.y1 as u32 - 1;

    let top = get(x, y - 1);
    let top_right = get(x + 1, y - 1);
    let center = get(x, y);
    let right = get(x + 1, y);

    return_normal(
        2. / 3.,
        -top + top_right - 2 * center + 2 * right,
        2. / 3.,
        -2 * top - top_right + 2 * center + right,
    )
}

/// Computes and returns the normal vector for the bottom row pixels for light filters.
#[inline]
pub fn bottom_row_normal(surface: &SharedImageSurface, bounds: IRect, x: u32) -> Normal {
    assert!(x as i32 > bounds.x0);
    assert!((x as i32) + 1 < bounds.x1);
    assert!(bounds.y1 >= bounds.y0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let y = bounds.y1 as u32 - 1;

    let top_left = get(x - 1, y - 1);
    let top = get(x, y - 1);
    let top_right = get(x + 1, y - 1);
    let left = get(x - 1, y);
    let center = get(x, y);
    let right = get(x + 1, y);

    return_normal(
        1. / 3.,
        -top_left + top_right - 2 * left + 2 * right,
        1. / 2.,
        -top_left - 2 * top - top_right + left + 2 * center + right,
    )
}

/// Computes and returns the normal vector for the bottom right pixel for light filters.
#[inline]
pub fn bottom_right_normal(surface: &SharedImageSurface, bounds: IRect) -> Normal {
    // Surface needs to be at least 2×2.
    assert!(bounds.x1 >= bounds.x0 + 2);
    assert!(bounds.y1 >= bounds.y0 + 2);

    let get = |x, y| i16::from(surface.get_pixel(x, y).a);
    let x = bounds.x1 as u32 - 1;
    let y = bounds.y1 as u32 - 1;

    let top_left = get(x - 1, y - 1);
    let top = get(x, y - 1);
    let left = get(x - 1, y);
    let center = get(x, y);

    return_normal(
        2. / 3.,
        -top_left + top - 2 * left + 2 * center,
        2. / 3.,
        -top_left - 2 * top + left + 2 * center,
    )
}
