use std::ptr;

use cairo::{self, ImageSurface};
use gdk_pixbuf::{Colorspace, Pixbuf};
use gdk_pixbuf_sys;
use glib::translate::*;
use glib_sys;
use libc;

use error::{set_gerror, RenderingError};
use handle::{get_rust_handle, rsvg_handle_rust_new_from_gfile_sync, Handle, RsvgDimensionData};
use rect::IRect;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
};

// Pixbuf::new() doesn't return out-of-memory errors properly
// See https://github.com/gtk-rs/gdk-pixbuf/issues/96
fn pixbuf_new(width: i32, height: i32) -> Result<Pixbuf, RenderingError> {
    unsafe {
        let raw_pixbuf = gdk_pixbuf_sys::gdk_pixbuf_new(
            Colorspace::Rgb.to_glib(),
            true.to_glib(),
            8,
            width,
            height,
        );

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

enum SizeKind {
    Zoom,
    WidthHeight,
    WidthHeightMax,
    ZoomMax,
}

struct SizeMode {
    kind: SizeKind,
    x_zoom: f64,
    y_zoom: f64,
    width: i32,
    height: i32,
}

fn get_final_size(dimensions: &RsvgDimensionData, size_mode: &SizeMode) -> (i32, i32) {
    let in_width = dimensions.width;
    let in_height = dimensions.height;

    let mut out_width;
    let mut out_height;

    match size_mode.kind {
        SizeKind::Zoom => {
            out_width = (size_mode.x_zoom * f64::from(in_width) + 0.5).floor() as i32;
            out_height = (size_mode.y_zoom * f64::from(in_height) + 0.5).floor() as i32;
        }

        SizeKind::ZoomMax => {
            out_width = (size_mode.x_zoom * f64::from(in_width) + 0.5).floor() as i32;
            out_height = (size_mode.y_zoom * f64::from(in_height) + 0.5).floor() as i32;

            if out_width > size_mode.width || out_height > size_mode.height {
                let zoom_x = f64::from(size_mode.width) / f64::from(out_width);
                let zoom_y = f64::from(size_mode.height) / f64::from(out_height);
                let zoom = zoom_x.min(zoom_y);

                out_width = (zoom * f64::from(out_width) + 0.5) as i32;
                out_height = (zoom * f64::from(out_height) + 0.5) as i32;
            }
        }

        SizeKind::WidthHeightMax => {
            let zoom_x = f64::from(size_mode.width) / f64::from(in_width);
            let zoom_y = f64::from(size_mode.height) / f64::from(in_height);

            let zoom = zoom_x.min(zoom_y);

            out_width = (zoom * f64::from(in_width) + 0.5) as i32;
            out_height = (zoom * f64::from(in_height) + 0.5) as i32;
        }

        SizeKind::WidthHeight => {
            if size_mode.width != -1 {
                out_width = size_mode.width;
            } else {
                out_width = in_width;
            }

            if size_mode.height != -1 {
                out_height = size_mode.height;
            } else {
                out_height = in_height;
            }
        }
    }

    (out_width, out_height)
}

fn render_to_pixbuf_at_size(
    handle: &Handle,
    dimensions: &RsvgDimensionData,
    width: i32,
    height: i32,
) -> Result<Pixbuf, RenderingError> {
    let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)?;

    {
        let cr = cairo::Context::new(&surface);
        cr.scale(
            f64::from(width) / f64::from(dimensions.width),
            f64::from(height) / f64::from(dimensions.height),
        );
        handle.render_cairo_sub(&cr, None)?;
    }

    let shared_surface = SharedImageSurface::new(surface, SurfaceType::SRgb)?;

    pixbuf_from_surface(&shared_surface)
}

fn pixbuf_from_file_with_size_mode(
    filename: *const libc::c_char,
    size_mode: &SizeMode,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    unsafe {
        let file = gio_sys::g_file_new_for_path(filename);

        let handle = rsvg_handle_rust_new_from_gfile_sync(file, 0, ptr::null_mut(), error);

        gobject_sys::g_object_unref(file as *mut _);

        if handle.is_null() {
            return ptr::null_mut();
        }

        let rhandle = get_rust_handle(handle);

        let raw_pixbuf = rhandle
            .get_dimensions()
            .and_then(|dimensions| {
                let (width, height) = get_final_size(&dimensions, size_mode);

                render_to_pixbuf_at_size(rhandle, &dimensions, width, height)
            })
            .and_then(|pixbuf| Ok(pixbuf.to_glib_full()))
            .map_err(|e| set_gerror(error, 0, &format!("{}", e)))
            .unwrap_or(ptr::null_mut());

        gobject_sys::g_object_unref(handle as *mut _);

        raw_pixbuf
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_pixbuf_from_file_at_size(
    filename: *const libc::c_char,
    width: i32,
    height: i32,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::WidthHeight,
            x_zoom: 0.0,
            y_zoom: 0.0,
            width,
            height,
        },
        error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_pixbuf_from_file_at_zoom(
    filename: *const libc::c_char,
    x_zoom: f64,
    y_zoom: f64,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::Zoom,
            x_zoom,
            y_zoom,
            width: 0,
            height: 0,
        },
        error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_pixbuf_from_file_at_zoom_with_max(
    filename: *const libc::c_char,
    x_zoom: f64,
    y_zoom: f64,
    width: i32,
    height: i32,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::ZoomMax,
            x_zoom,
            y_zoom,
            width,
            height,
        },
        error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_pixbuf_from_file_at_max_size(
    filename: *const libc::c_char,
    width: i32,
    height: i32,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::WidthHeightMax,
            x_zoom: 0.0,
            y_zoom: 0.0,
            width,
            height,
        },
        error,
    )
}
