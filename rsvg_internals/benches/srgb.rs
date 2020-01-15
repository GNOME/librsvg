#[macro_use]
extern crate criterion;
use criterion::{black_box, Criterion};

use rsvg_internals::rect::IRect;
use rsvg_internals::surface_utils::{
    shared_surface::{ExclusiveImageSurface, SurfaceType},
    srgb::{linearize, map_unpremultiplied_components_loop},
    ImageSurfaceDataExt, Pixel,
};

const SURFACE_SIDE: i32 = 512;
const BOUNDS: IRect = IRect {
    x0: 64,
    y0: 32,
    x1: 448,
    y1: 480,
};

fn bench_srgb_linearization(c: &mut Criterion) {
    c.bench_function("srgb map_unpremultiplied_components", |b| {
        let mut surface =
            ExclusiveImageSurface::new(SURFACE_SIDE, SURFACE_SIDE, SurfaceType::LinearRgb).unwrap();

        let surface_stride = surface.stride() as usize;

        {
            let mut surface_data = surface.get_data();

            // Fill the surface with non-zero alpha (otherwise linearization is a no-op).
            for y in BOUNDS.y_range() {
                for x in BOUNDS.x_range() {
                    let pixel = Pixel {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 127,
                    };

                    surface_data.set_pixel(surface_stride, pixel, x as u32, y as u32);
                }
            }
        }

        let surface = surface.share().unwrap();
        let mut output_surface =
            ExclusiveImageSurface::new(SURFACE_SIDE, SURFACE_SIDE, SurfaceType::SRgb).unwrap();
        let bounds = black_box(BOUNDS);

        b.iter(|| {
            map_unpremultiplied_components_loop(&surface, &mut output_surface, bounds, linearize);
        })
    });
}

criterion_group!(benches, bench_srgb_linearization);
criterion_main!(benches);
