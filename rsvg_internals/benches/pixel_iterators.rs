#[macro_use]
extern crate criterion;
use criterion::{black_box, Criterion};

extern crate cairo;
extern crate cairo_sys;
extern crate rsvg_internals;

use rsvg_internals::filters::context::IRect;
use rsvg_internals::surface_utils::{
    iterators::Pixels,
    shared_surface::{SharedImageSurface, SurfaceType},
};

const SURFACE_SIDE: i32 = 512;
const BOUNDS: IRect = IRect {
    x0: 64,
    y0: 32,
    x1: 448,
    y1: 480,
};

fn bench_pixel_iterators(c: &mut Criterion) {
    c.bench_function("pixel_iterators straightforward", |b| {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let stride = surface.get_stride();
        let data = surface.get_data().unwrap();

        let bounds = black_box(BOUNDS);

        b.iter(|| {
            let mut r = 0usize;
            let mut g = 0usize;
            let mut b = 0usize;
            let mut a = 0usize;

            for y in bounds.y0..bounds.y1 {
                for x in bounds.x0..bounds.x1 {
                    let base = (y * stride + x * 4) as usize;

                    r += data[base + 0] as usize;
                    g += data[base + 1] as usize;
                    b += data[base + 2] as usize;
                    a += data[base + 3] as usize;
                }
            }

            (r, g, b, a)
        })
    });

    c.bench_function("pixel_iterators get_pixel", |b| {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let surface = SharedImageSurface::new(surface, SurfaceType::SRgb).unwrap();

        let bounds = black_box(BOUNDS);

        b.iter(|| {
            let mut r = 0usize;
            let mut g = 0usize;
            let mut b = 0usize;
            let mut a = 0usize;

            for y in bounds.y0..bounds.y1 {
                for x in bounds.x0..bounds.x1 {
                    let pixel = surface.get_pixel(x as u32, y as u32);

                    r += pixel.r as usize;
                    g += pixel.g as usize;
                    b += pixel.b as usize;
                    a += pixel.a as usize;
                }
            }

            (r, g, b, a)
        })
    });

    c.bench_function("pixel_iterators pixels", |b| {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let data = SharedImageSurface::new(surface, SurfaceType::SRgb).unwrap();

        let bounds = black_box(BOUNDS);

        b.iter(|| {
            let mut r = 0usize;
            let mut g = 0usize;
            let mut b = 0usize;
            let mut a = 0usize;

            for (_x, _y, pixel) in Pixels::new(&data, bounds) {
                r += pixel.r as usize;
                g += pixel.g as usize;
                b += pixel.b as usize;
                a += pixel.a as usize;
            }

            (r, g, b, a)
        })
    });
}

criterion_group!(benches, bench_pixel_iterators);
criterion_main!(benches);
