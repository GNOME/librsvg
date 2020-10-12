use self::rsvg_internals::surface_utils::{
    iterators::Pixels,
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt, Pixel, PixelOps,
};
use self::rsvg_internals::{IRect, RenderingError};
use rsvg_internals;

use rgb::ComponentMap;

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
    let emphasize_component = |c| {
        // emphasize
        let mut c = c as u32 * 4;
        // make sure it's visible
        if c > 0 {
            c += 128;
        }
        c.min(255) as u8
    };
    p.map(emphasize_component)
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
