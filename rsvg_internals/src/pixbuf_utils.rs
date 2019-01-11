use glib::translate::*;

use error::RenderingError;
use gdk_pixbuf::{Colorspace, Pixbuf};
use gdk_pixbuf_sys as ffi;
use rect::IRect;
use surface_utils::{iterators::Pixels, shared_surface::SharedImageSurface};

// Pixbuf::new() doesn't return out-of-memory errors properly
// See https://github.com/gtk-rs/gdk-pixbuf/issues/96
fn pixbuf_new(width: i32, height: i32) -> Result<Pixbuf, RenderingError> {
    unsafe {
        let raw_pixbuf =
            ffi::gdk_pixbuf_new(Colorspace::Rgb.to_glib(), true.to_glib(), 8, width, height);

        if raw_pixbuf.is_null() {
            return Err(RenderingError::OutOfMemory);
        }

        Ok(from_glib_full(raw_pixbuf))
    }
}

pub fn pixbuf_from_surface(surface: &SharedImageSurface) -> Result<Pixbuf, RenderingError> {
    let width = surface.width();
    let height = surface.height();

    let pixbuf = pixbuf_new(width as i32, height as i32)?;

    let bounds = IRect {
        x0: 0,
        y0: 0,
        x1: width,
        y1: height,
    };

    for (x, y, pixel) in Pixels::new(&surface, bounds) {
        let (r, g, b, a) = if pixel.a == 0 {
            (0, 0, 0, 0)
        } else {
            let pixel = pixel.unpremultiply();
            (pixel.r, pixel.g, pixel.b, pixel.a)
        };

        pixbuf.put_pixel(x as i32, y as i32, r, g, b, a);
    }

    Ok(pixbuf)
}
