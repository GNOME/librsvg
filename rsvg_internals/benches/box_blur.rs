#[macro_use]
extern crate criterion;
use criterion::Criterion;

extern crate cairo;
extern crate cairo_sys;
extern crate rsvg_internals;

use rsvg_internals::filters::context::IRect;
use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

const SURFACE_SIDE: i32 = 512;
const BOUNDS: IRect = IRect {
    x0: 64,
    y0: 64,
    x1: 64 + 64,
    y1: 64 + 64,
};

fn bench_box_blur(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "box_blur 9",
        |b, &(vertical, alpha_only)| {
            let input_surface =
                cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE)
                    .unwrap();
            let surface_type = if alpha_only {
                SurfaceType::AlphaOnly
            } else {
                SurfaceType::SRgb
            };
            let input_surface = SharedImageSurface::new(input_surface, surface_type).unwrap();

            let mut output_surface =
                cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE)
                    .unwrap();
            const KERNEL_SIZE: usize = 9;

            b.iter(|| {
                input_surface.box_blur_loop(
                    &mut output_surface,
                    BOUNDS,
                    KERNEL_SIZE,
                    KERNEL_SIZE / 2,
                    vertical,
                )
            })
        },
        vec![(false, false), (false, true), (true, false), (true, true)],
    );
}

criterion_group!(benches, bench_box_blur);
criterion_main!(benches);
