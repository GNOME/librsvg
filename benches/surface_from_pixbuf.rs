use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use gdk_pixbuf::{Colorspace, Pixbuf};

use librsvg::surface_utils::shared_surface::SharedImageSurface;

fn bench_surface_from_pixbuf(c: &mut Criterion) {
    let mut group = c.benchmark_group("surface_from_pixbuf");

    for input in [false, true].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", input)),
            input,
            |b, alpha| {
                let pixbuf = Pixbuf::new(Colorspace::Rgb, *alpha, 8, 256, 256).unwrap();

                // Fill the surface with interesting data
                for y in 0..pixbuf.width() {
                    for x in 0..pixbuf.height() {
                        pixbuf.put_pixel(
                            x as u32,
                            y as u32,
                            x as u8,
                            y as u8,
                            x.max(y) as u8,
                            0xff,
                        );
                    }
                }

                b.iter(|| SharedImageSurface::from_pixbuf(&pixbuf, None, None).unwrap())
            },
        );
    }
}

criterion_group!(benches, bench_surface_from_pixbuf);
criterion_main!(benches);
