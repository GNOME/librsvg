#[macro_use]
extern crate criterion;
use criterion::Criterion;

extern crate cairo;
extern crate cairo_sys;
extern crate nalgebra;
extern crate rsvg_internals;

use nalgebra::{Matrix3, Vector2};

use rsvg_internals::filters::{
    context::IRect,
    light::{
        bottom_left_normal,
        bottom_right_normal,
        bottom_row_normal,
        interior_normal,
        left_column_normal,
        right_column_normal,
        top_left_normal,
        top_right_normal,
        top_row_normal,
        Normal,
    },
};
use rsvg_internals::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::{SharedImageSurface, SurfaceType},
    EdgeMode,
};

/// Computes and returns the normal vector for the light filters.
fn normal(surface: &SharedImageSurface, bounds: IRect, x: u32, y: u32) -> Normal {
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
                0,  0, 0,
                0, -2, 2,
                0, -1, 1,
            ),
            2. / 3.,
            Matrix3::new(
                0,  0,  0,
                0, -2, -1,
                0,  2,  1,
            ),
        ),
        (x, y) if (x + 1, y) == (bounds.x1, bounds.y0) => (
            2. / 3.,
            Matrix3::new(
                0,  0, 0,
                -2, 2, 0,
                -1, 1, 0,
            ),
            2. / 3.,
            Matrix3::new(
                 0,  0, 0,
                -1, -2, 0,
                 1,  2, 0,
            ),
        ),
        (x, y) if (x, y + 1) == (bounds.x0, bounds.y1) => (
            2. / 3.,
            Matrix3::new(
                0, -1, 1,
                0, -2, 2,
                0,  0, 0,
            ),
            2. / 3.,
            Matrix3::new(
                0, -2, -1,
                0,  2,  1,
                0,  0,  0,
            ),
        ),
        (x, y) if (x + 1, y + 1) == (bounds.x1, bounds.y1) => (
            2. / 3.,
            Matrix3::new(
                -1, 1, 0,
                -2, 2, 0,
                 0, 0, 0,
            ),
            2. / 3.,
            Matrix3::new(
                -1, -2, 0,
                 1,  2, 0,
                 0,  0, 0,
            ),
        ),
        (_, y) if y == bounds.y0 => (
            1. / 3.,
            Matrix3::new(
                 0, 0, 0,
                -2, 0, 2,
                -1, 0, 1,
            ),
            1. / 2.,
            Matrix3::new(
                 0,  0,  0,
                -1, -2, -1,
                 1,  2,  1,
            ),
        ),
        (x, _) if x == bounds.x0 => (
            1. / 2.,
            Matrix3::new(
                0, -1, 1,
                0, -2, 2,
                0, -1, 1,
            ),
            1. / 3.,
            Matrix3::new(
                0, -2, -1,
                0,  0,  0,
                0,  2,  1,
            ),
        ),
        (x, _) if x + 1 == bounds.x1 => (
            1. / 2.,
            Matrix3::new(
                -1, 1, 0,
                -2, 2, 0,
                -1, 1, 0,
            ),
            1. / 3.,
            Matrix3::new(
                -1, -2, 0,
                 0,  0, 0,
                 1,  2, 0,
            ),
        ),
        (_, y) if y + 1 == bounds.y1 => (
            1. / 3.,
            Matrix3::new(
                -1, 0, 1,
                -2, 0, 2,
                 0, 0, 0,
            ),
            1. / 2.,
            Matrix3::new(
                -1, -2, -1,
                 1,  2,  1,
                 0,  0,  0,
            ),
        ),
        _ => (
            1. / 4.,
            Matrix3::new(
                -1, 0, 1,
                -2, 0, 2,
                -1, 0, 1,
            ),
            1. / 4.,
            Matrix3::new(
                -1, -2, -1,
                 0,  0,  0,
                 1,  2,  1,
            ),
        ),
    };

    let kernel_bounds = IRect {
        x0: x as i32 - 1,
        y0: y as i32 - 1,
        x1: x as i32 + 2,
        y1: y as i32 + 2,
    };

    let mut nx = 0;
    let mut ny = 0;
    for (x, y, pixel) in PixelRectangle::new(surface, bounds, kernel_bounds, EdgeMode::None) {
        let kernel_x = (x - kernel_bounds.x0) as usize;
        let kernel_y = (y - kernel_bounds.y0) as usize;

        nx += i16::from(pixel.a) * kx[(kernel_y, kernel_x)];
        ny += i16::from(pixel.a) * ky[(kernel_y, kernel_x)];
    }

    // Negative nx and ny to account for the different coordinate system.
    Normal {
        factor: Vector2::new(factor_x, factor_y),
        normal: Vector2::new(-nx, -ny),
    }
}

const SURFACE_SIDE: i32 = 512;
const BOUNDS: IRect = IRect {
    x0: 64,
    y0: 64,
    x1: 64 + 64,
    y1: 64 + 64,
};

fn bench_normal(c: &mut Criterion) {
    c.bench_function("normal", |b| {
        let input_surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let input_surface = SharedImageSurface::new(input_surface, SurfaceType::SRgb).unwrap();

        b.iter(|| {
            let mut z = 0;
            for (x, y, _pixel) in Pixels::new(&input_surface, BOUNDS) {
                let n = normal(&input_surface, BOUNDS, x, y);
                z += n.normal.x;
            }
            z
        })
    });

    c.bench_function("normal unrolled", |b| {
        let input_surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let input_surface = SharedImageSurface::new(input_surface, SurfaceType::SRgb).unwrap();

        b.iter(|| {
            let mut z = 0;

            // Top left.
            {
                let n = top_left_normal(&input_surface, BOUNDS);
                z += n.normal.x;
            }

            // Top right.
            {
                let n = top_right_normal(&input_surface, BOUNDS);
                z += n.normal.x;
            }

            // Bottom left.
            {
                let n = bottom_left_normal(&input_surface, BOUNDS);
                z += n.normal.x;
            }

            // Bottom right.
            {
                let n = bottom_right_normal(&input_surface, BOUNDS);
                z += n.normal.x;
            }

            // Top row.
            for x in BOUNDS.x0 as u32 + 1..BOUNDS.x1 as u32 - 1 {
                let n = top_row_normal(&input_surface, BOUNDS, x);
                z += n.normal.x;
            }

            // Bottom row.
            for x in BOUNDS.x0 as u32 + 1..BOUNDS.x1 as u32 - 1 {
                let n = bottom_row_normal(&input_surface, BOUNDS, x);
                z += n.normal.x;
            }

            // Left column.
            for y in BOUNDS.y0 as u32 + 1..BOUNDS.y1 as u32 - 1 {
                let n = left_column_normal(&input_surface, BOUNDS, y);
                z += n.normal.x;
            }

            // Right column.
            for y in BOUNDS.y0 as u32 + 1..BOUNDS.y1 as u32 - 1 {
                let n = right_column_normal(&input_surface, BOUNDS, y);
                z += n.normal.x;
            }

            // Interior pixels.
            for y in BOUNDS.y0 as u32 + 1..BOUNDS.y1 as u32 - 1 {
                for x in BOUNDS.x0 as u32 + 1..BOUNDS.x1 as u32 - 1 {
                    let n = interior_normal(&input_surface, BOUNDS, x, y);
                    z += n.normal.x;
                }
            }

            z
        })
    });
}

criterion_group!(benches, bench_normal);
criterion_main!(benches);
