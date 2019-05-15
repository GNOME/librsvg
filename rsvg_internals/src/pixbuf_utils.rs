use std::path::PathBuf;
use std::ptr;

use cairo::{self, ImageSurface};
use gdk_pixbuf::{Colorspace, Pixbuf};
use gdk_pixbuf_sys;
use gio;
use gio::prelude::*;
use glib::translate::*;
use glib_sys;
use libc;
use url::Url;

use crate::c_api::{RsvgDimensionData, SizeCallback};
use crate::dpi::Dpi;
use crate::error::{set_gerror, LoadingError, RenderingError};
use crate::handle::{Handle, LoadOptions};
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
};

// Pixbuf::new() doesn't return out-of-memory errors properly
// See https://github.com/gtk-rs/gdk-pixbuf/issues/96
fn pixbuf_new(width: i32, height: i32) -> Result<Pixbuf, RenderingError> {
    assert!(width > 0 && height > 0);

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

pub fn empty_pixbuf() -> Result<Pixbuf, RenderingError> {
    // GdkPixbuf does not allow zero-sized pixbufs, but Cairo allows zero-sized
    // surfaces.  In this case, return a 1-pixel transparent pixbuf.

    unsafe {
        let raw_pixbuf =
            gdk_pixbuf_sys::gdk_pixbuf_new(Colorspace::Rgb.to_glib(), true.to_glib(), 8, 1, 1);

        if raw_pixbuf.is_null() {
            return Err(RenderingError::OutOfMemory);
        }

        let pixbuf: Pixbuf = from_glib_full(raw_pixbuf);
        pixbuf.put_pixel(0, 0, 0, 0, 0, 0);

        Ok(pixbuf)
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
    dpi: Dpi,
) -> Result<Pixbuf, RenderingError> {
    if width == 0 || height == 0 {
        return empty_pixbuf();
    }

    let surface = ImageSurface::create(cairo::Format::ARgb32, width, height)?;

    {
        let cr = cairo::Context::new(&surface);
        cr.scale(
            f64::from(width) / f64::from(dimensions.width),
            f64::from(height) / f64::from(dimensions.height),
        );
        handle.render_cairo_sub(&cr, None, dpi, &SizeCallback::default(), false)?;
    }

    let shared_surface = SharedImageSurface::new(surface, SurfaceType::SRgb)?;

    pixbuf_from_surface(&shared_surface)
}

fn get_default_dpi() -> Dpi {
    // This is ugly, but it preserves the C API semantics of
    //
    //   rsvg_set_default_dpi(...);
    //   pixbuf = rsvg_pixbuf_from_file(...);
    //
    // Passing negative numbers here means that the global default DPI will be used.
    Dpi::new(-1.0, -1.0)
}

fn url_from_file(file: &gio::File) -> Result<Url, LoadingError> {
    if let Some(uri) = file.get_uri() {
        Ok(Url::parse(&uri).map_err(|_| LoadingError::BadUrl)?)
    } else {
        Err(LoadingError::BadUrl)
    }
}

fn pixbuf_from_file_with_size_mode(
    filename: *const libc::c_char,
    size_mode: &SizeMode,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    let dpi = get_default_dpi();

    unsafe {
        let path = PathBuf::from_glib_none(filename);
        let file = gio::File::new_for_path(path);

        let base_url = match url_from_file(&file) {
            Ok(url) => url,
            Err(e) => {
                set_gerror(error, 0, &format!("{}", e));
                return ptr::null_mut();
            }
        };

        let load_options = LoadOptions::new(Some(base_url));

        let cancellable: Option<&gio::Cancellable> = None;
        let handle = match file
            .read(cancellable)
            .map_err(|e| LoadingError::from(e))
            .and_then(|stream| Handle::from_stream(&load_options, &stream, None))
        {
            Ok(handle) => handle,
            Err(e) => {
                set_gerror(error, 0, &format!("{}", e));
                return ptr::null_mut();
            }
        };

        handle
            .get_dimensions(dpi, &SizeCallback::default(), false)
            .and_then(|dimensions| {
                let (width, height) = get_final_size(&dimensions, size_mode);

                render_to_pixbuf_at_size(&handle, &dimensions, width, height, dpi)
            })
            .and_then(|pixbuf| Ok(pixbuf.to_glib_full()))
            .unwrap_or_else(|e| {
                set_gerror(error, 0, &format!("{}", e));
                ptr::null_mut()
            })
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
