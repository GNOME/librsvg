//! Light filters and nodes.
use nalgebra::{Matrix3, Vector3};

use filters::context::IRect;
use surface_utils::{iterators::PixelRectangle, shared_surface::SharedImageSurface, EdgeMode};

pub mod light_source;
pub mod lighting;

/// Computes and returns the normal vector for the light filters.
fn normal(
    surface: &SharedImageSurface,
    bounds: IRect,
    x: u32,
    y: u32,
    surface_scale: f64,
) -> Vector3<f64> {
    assert!(x as i32 >= bounds.x0);
    assert!(y as i32 >= bounds.y0);
    assert!((x as i32) < bounds.x1);
    assert!((y as i32) < bounds.y1);

    // Get the correct sobel kernel and factor for the pixel position.
    // Performance note: it's possible to replace the matrices with normal arrays.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let (factor_x, kx, factor_y, ky) = match (x as i32, y as i32) {
        (x, y) if (x, y) == (bounds.x0, bounds.y0) => (
            2. / 3.,
            Matrix3::new(
                0.,  0., 0.,
                0., -2., 2.,
                0., -1., 1.,
            ),
            2. / 3.,
            Matrix3::new(
                0.,  0.,  0.,
                0., -2., -1.,
                0.,  2.,  1.,
            ),
        ),
        (x, y) if (x + 1, y) == (bounds.x1, bounds.y0) => (
            2. / 3.,
            Matrix3::new(
                0.,  0., 0.,
                -2., 2., 0.,
                -1., 1., 0.,
            ),
            2. / 3.,
            Matrix3::new(
                 0.,  0., 0.,
                -1., -2., 0.,
                 1.,  2., 0.,
            ),
        ),
        (x, y) if (x, y + 1) == (bounds.x0, bounds.y1) => (
            2. / 3.,
            Matrix3::new(
                0., -1., 1.,
                0., -2., 2.,
                0.,  0., 0.,
            ),
            2. / 3.,
            Matrix3::new(
                0., -2., -1.,
                0.,  2.,  1.,
                0.,  0.,  0.,
            ),
        ),
        (x, y) if (x + 1, y + 1) == (bounds.x1, bounds.y1) => (
            2. / 3.,
            Matrix3::new(
                -1., 1., 0.,
                -2., 2., 0.,
                 0., 0., 0.,
            ),
            2. / 3.,
            Matrix3::new(
                -1., -2., 0.,
                 1.,  2., 0.,
                 0.,  0., 0.,
            ),
        ),
        (_, y) if y == bounds.y0 => (
            1. / 3.,
            Matrix3::new(
                 0., 0., 0.,
                -2., 0., 2.,
                -1., 0., 1.,
            ),
            1. / 2.,
            Matrix3::new(
                 0.,  0.,  0.,
                -1., -2., -1.,
                 1.,  2.,  1.,
            ),
        ),
        (x, _) if x == bounds.x0 => (
            1. / 2.,
            Matrix3::new(
                0., -1., 1.,
                0., -2., 2.,
                0., -1., 1.,
            ),
            1. / 3.,
            Matrix3::new(
                0., -2., -1.,
                0.,  0.,  0.,
                0.,  2.,  1.,
            ),
        ),
        (x, _) if x + 1 == bounds.x1 => (
            1. / 2.,
            Matrix3::new(
                -1., 1., 0.,
                -2., 2., 0.,
                -1., 1., 0.,
            ),
            1. / 3.,
            Matrix3::new(
                -1., -2., 0.,
                 0.,  0., 0.,
                 1.,  2., 0.,
            ),
        ),
        (_, y) if y + 1 == bounds.y1 => (
            1. / 3.,
            Matrix3::new(
                -1., 0., 1.,
                -2., 0., 2.,
                 0., 0., 0.,
            ),
            1. / 2.,
            Matrix3::new(
                -1., -2., -1.,
                 1.,  2.,  1.,
                 0.,  0.,  0.,
            ),
        ),
        _ => (
            1. / 4.,
            Matrix3::new(
                -1., 0., 1.,
                -2., 0., 2.,
                -1., 0., 1.,
            ),
            1. / 4.,
            Matrix3::new(
                -1., -2., -1.,
                 0.,  0.,  0.,
                 1.,  2.,  1.,
            ),
        ),
    };

    let kernel_bounds = IRect {
        x0: x as i32 - 1,
        y0: y as i32 - 1,
        x1: x as i32 + 2,
        y1: y as i32 + 2,
    };

    let mut nx = 0.0;
    let mut ny = 0.0;
    for (x, y, pixel) in PixelRectangle::new(surface, bounds, kernel_bounds, EdgeMode::None) {
        let kernel_x = (x - kernel_bounds.x0) as usize;
        let kernel_y = (y - kernel_bounds.y0) as usize;
        let alpha = f64::from(pixel.a) / 255.0;

        nx += alpha * kx[(kernel_y, kernel_x)];
        ny += alpha * ky[(kernel_y, kernel_x)];
    }

    nx *= factor_x * surface_scale;
    ny *= factor_y * surface_scale;

    // Negative nx and ny to account for the different coordinate system.
    let mut n = Vector3::new(-nx, -ny, 1.0);
    n.normalize_mut();
    n
}
