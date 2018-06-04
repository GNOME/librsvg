#![feature(test)]

extern crate cairo;
extern crate cairo_sys;
extern crate rsvg_internals;
extern crate test;

#[cfg(test)]
mod tests {
    use super::*;
    use rsvg_internals::filters::context::IRect;
    use rsvg_internals::filters::iterators;
    use test::Bencher;

    const SURFACE_SIDE: i32 = 512;
    const BOUNDS: IRect = IRect {
        x0: 64,
        y0: 32,
        x1: 448,
        y1: 480,
    };

    #[bench]
    fn bench_straightforward(b: &mut Bencher) {
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let stride = surface.get_stride();
        let data = surface.get_data().unwrap();

        b.iter(|| {
            let mut r = 0usize;
            let mut g = 0usize;
            let mut b = 0usize;
            let mut a = 0usize;

            for y in BOUNDS.y0..BOUNDS.y1 {
                for x in BOUNDS.x0..BOUNDS.x1 {
                    let base = (y * stride + x * 4) as usize;

                    r += data[base + 0] as usize;
                    g += data[base + 1] as usize;
                    b += data[base + 2] as usize;
                    a += data[base + 3] as usize;
                }
            }

            (r, g, b, a)
        })
    }

    #[bench]
    fn bench_straightforward_getpixel(b: &mut Bencher) {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let data = iterators::ImageSurfaceDataShared::new(&surface).unwrap();

        b.iter(|| {
            let mut r = 0usize;
            let mut g = 0usize;
            let mut b = 0usize;
            let mut a = 0usize;

            for y in BOUNDS.y0..BOUNDS.y1 {
                for x in BOUNDS.x0..BOUNDS.x1 {
                    let pixel = data.get_pixel(x as usize, y as usize);

                    r += pixel.r as usize;
                    g += pixel.g as usize;
                    b += pixel.b as usize;
                    a += pixel.a as usize;
                }
            }

            (r, g, b, a)
        })
    }

    #[bench]
    fn bench_pixels(b: &mut Bencher) {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, SURFACE_SIDE, SURFACE_SIDE).unwrap();
        let data = iterators::ImageSurfaceDataShared::new(&surface).unwrap();

        b.iter(|| {
            let mut r = 0usize;
            let mut g = 0usize;
            let mut b = 0usize;
            let mut a = 0usize;

            for (_x, _y, pixel) in iterators::Pixels::new(data, BOUNDS) {
                r += pixel.r as usize;
                g += pixel.g as usize;
                b += pixel.b as usize;
                a += pixel.a as usize;
            }

            (r, g, b, a)
        })
    }
}
