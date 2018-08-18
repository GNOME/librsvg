#[macro_use]
extern crate criterion;
use criterion::{black_box, Criterion};

extern crate cairo;
extern crate cairo_sys;
extern crate rsvg_internals;

use rsvg_internals::filters::context::IRect;
use rsvg_internals::srgb::{linearize, map_unpremultiplied_components_loop};
use rsvg_internals::surface_utils::{
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt,
    Pixel,
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
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let mut output_surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();

        // Fill the surface with non-zero alpha (otherwise linearization is a no-op).
        let stride = surface.get_stride() as usize;
        {
            let mut data = surface.get_data().unwrap();
            for y in BOUNDS.y0..BOUNDS.y1 {
                for x in BOUNDS.x0..BOUNDS.x1 {
                    data.set_pixel(
                        stride,
                        Pixel {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 127,
                        },
                        x as u32,
                        y as u32,
                    );
                }
            }
        }

        let surface = SharedImageSurface::new(surface, SurfaceType::LinearRgb).unwrap();

        let bounds = black_box(BOUNDS);

        b.iter(|| {
            map_unpremultiplied_components_loop(&surface, &mut output_surface, bounds, linearize);
        })
    });
}

criterion_group!(benches, bench_srgb_linearization);
criterion_main!(benches);
