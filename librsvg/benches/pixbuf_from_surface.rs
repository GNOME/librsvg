use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

use rsvg_c_api::pixbuf_utils::pixbuf_from_surface;
use rsvg_internals::rect::IRect;
use rsvg_internals::surface_utils::{
    shared_surface::{ExclusiveImageSurface, SurfaceType},
    ImageSurfaceDataExt, Pixel,
};

const BOUNDS: IRect = IRect {
    x0: 0,
    y0: 0,
    x1: 256,
    y1: 256,
};

fn bench_pixbuf_from_surface(c: &mut Criterion) {
    c.bench_function("pixbuf_from_surface", |b| {
        let mut surface = ExclusiveImageSurface::new(256, 256, SurfaceType::SRgb).unwrap();

        // Fill the surface with interesting data
        surface.modify(&mut |data, stride| {
            for y in BOUNDS.y_range() {
                for x in BOUNDS.x_range() {
                    let pixel = Pixel {
                        r: x as u8,
                        g: y as u8,
                        b: x.max(y) as u8,
                        a: 255,
                    };

                    data.set_pixel(stride, pixel, x as u32, y as u32);
                }
            }
        });

        let surface = surface.share().unwrap();

        b.iter(|| {
            let _pixbuf = pixbuf_from_surface(&surface).unwrap();
        })
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = bench_pixbuf_from_surface,
);
criterion_main!(benches);
