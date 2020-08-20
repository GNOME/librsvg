use self::rsvg_internals::surface_utils::{
    iterators::Pixels,
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt, Pixel,
};
use self::rsvg_internals::{IRect, RenderingError};
use rsvg_internals;

pub enum BufferDiff {
    DifferentSizes,
    Diff(Diff),
}

pub struct Diff {
    pub num_pixels_changed: usize,
    pub max_diff: u8,
    pub surface: SharedImageSurface,
}

#[inline]
fn pixel_max(p: &Pixel) -> u8 {
    p.r.max(p.g).max(p.b).max(p.a)
}

#[inline]
fn emphasize(p: &Pixel) -> Pixel {
    let mut r = p.r as u32;
    let mut g = p.g as u32;
    let mut b = p.b as u32;
    let mut a = p.a as u32;

    // emphasize
    r = r * 4;
    g = g * 4;
    b = b * 4;
    a = a * 4;

    // make sure they are visible
    if r > 0 {
        r += 128;
    }

    if g > 0 {
        g += 128;
    }

    if b > 0 {
        b += 128;
    }

    if a > 0 {
        a += 128;
    }

    let r = r.min(255) as u8;
    let g = g.min(255) as u8;
    let b = b.min(255) as u8;
    let a = a.min(255) as u8;

    Pixel { r, g, b, a }
}

pub fn compare_surfaces(
    surf_a: &SharedImageSurface,
    surf_b: &SharedImageSurface,
) -> Result<BufferDiff, RenderingError> {
    let a_width = surf_a.width();
    let a_height = surf_a.height();

    let b_width = surf_b.width();
    let b_height = surf_b.height();

    if a_width != b_width || a_height != b_height {
        return Ok(BufferDiff::DifferentSizes);
    }

    let mut surf_diff = cairo::ImageSurface::create(cairo::Format::ARgb32, a_width, a_height)?;
    let diff_stride = surf_diff.get_stride() as usize;

    let mut num_pixels_changed = 0;
    let mut max_diff = 0;

    {
        let mut diff_data = surf_diff.get_data().unwrap();

        for ((xa, ya, pixel_a), (_, _, pixel_b)) in Pixels::new(surf_a).zip(Pixels::new(surf_b)) {
            if pixel_a != pixel_b {
                num_pixels_changed += 1;

                let pixel_diff = pixel_a.diff(&pixel_b);

                let pixel_max_diff = pixel_max(&pixel_diff);

                max_diff = max_diff.max(pixel_max_diff);

                let mut pixel_diff = emphasize(&pixel_diff);

                if pixel_diff.r == 0 && pixel_diff.g == 0 && pixel_diff.b == 0 {
                    // alpha only difference; convert alpha to gray

                    pixel_diff.r = pixel_diff.a;
                    pixel_diff.g = pixel_diff.a;
                    pixel_diff.b = pixel_diff.a;
                }

                pixel_diff.a = 255;

                diff_data.set_pixel(diff_stride, pixel_diff, xa, ya);
            } else {
                let black = Pixel {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                };
                diff_data.set_pixel(diff_stride, black, xa, ya);
            }
        }
    }

    let surface = SharedImageSurface::wrap(surf_diff, SurfaceType::SRgb)?;

    Ok(BufferDiff::Diff(Diff {
        num_pixels_changed,
        max_diff,
        surface,
    }))
}
