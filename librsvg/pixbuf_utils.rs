use std::path::PathBuf;
use std::ptr;

use gdk_pixbuf::{Colorspace, Pixbuf};
use gio::prelude::*;
use glib::translate::*;
use rgb::{FromSlice, RGBA8};
use url::Url;

use crate::c_api::checked_i32;

use rsvg_internals::{
    Dpi, Handle, LoadOptions, LoadingError, Pixel, RenderingError, SharedImageSurface, SurfaceType,
};

use crate::c_api::set_gerror;

fn pixbuf_new(width: i32, height: i32) -> Result<Pixbuf, RenderingError> {
    assert!(width > 0 && height > 0);

    Pixbuf::new(Colorspace::Rgb, true, 8, width, height).ok_or(RenderingError::OutOfMemory)
}

pub fn empty_pixbuf() -> Result<Pixbuf, RenderingError> {
    // GdkPixbuf does not allow zero-sized pixbufs, but Cairo allows zero-sized
    // surfaces.  In this case, return a 1-pixel transparent pixbuf.

    let pixbuf = pixbuf_new(1, 1)?;
    pixbuf.put_pixel(0, 0, 0, 0, 0, 0);

    Ok(pixbuf)
}

pub fn pixbuf_from_surface(surface: &SharedImageSurface) -> Result<Pixbuf, RenderingError> {
    let width = surface.width();
    let height = surface.height();

    let pixbuf = pixbuf_new(width, height)?;
    assert!(pixbuf.get_colorspace() == Colorspace::Rgb);
    assert!(pixbuf.get_bits_per_sample() == 8);
    assert!(pixbuf.get_n_channels() == 4);

    let pixels = unsafe { pixbuf.get_pixels() };
    let width = width as usize;
    let height = height as usize;
    let stride = pixbuf.get_rowstride() as usize;
    let width_in_bytes = width * 4;
    assert!(width_in_bytes <= stride);

    // We use chunks_mut(), not chunks_exact_mut(), because gdk-pixbuf tends
    // to make the last row *not* have the full stride (i.e. it is
    // only as wide as the pixels in that row).
    let pixbuf_rows = pixels.chunks_mut(stride).take(height);

    for (src_row, dest_row) in surface.rows().zip(pixbuf_rows) {
        let row: &mut [RGBA8] = dest_row[..width_in_bytes].as_rgba_mut();

        for (src, dest) in src_row.iter().zip(row.iter_mut()) {
            let (r, g, b, a) = if src.a == 0 {
                (0, 0, 0, 0)
            } else {
                let pixel = Pixel {
                    r: src.r,
                    g: src.g,
                    b: src.b,
                    a: src.a,
                }
                .unpremultiply();

                (pixel.r, pixel.g, pixel.b, pixel.a)
            };

            dest.r = r;
            dest.g = g;
            dest.b = b;
            dest.a = a;
        }
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

fn get_final_size(in_width: f64, in_height: f64, size_mode: &SizeMode) -> (f64, f64) {
    if in_width == 0.0 || in_height == 0.0 {
        return (0.0, 0.0);
    }

    let mut out_width;
    let mut out_height;

    match size_mode.kind {
        SizeKind::Zoom => {
            out_width = size_mode.x_zoom * in_width;
            out_height = size_mode.y_zoom * in_height;
        }

        SizeKind::ZoomMax => {
            out_width = size_mode.x_zoom * in_width;
            out_height = size_mode.y_zoom * in_height;

            if out_width > f64::from(size_mode.width) || out_height > f64::from(size_mode.height) {
                let zoom_x = f64::from(size_mode.width) / out_width;
                let zoom_y = f64::from(size_mode.height) / out_height;
                let zoom = zoom_x.min(zoom_y);

                out_width *= zoom;
                out_height *= zoom;
            }
        }

        SizeKind::WidthHeightMax => {
            let zoom_x = f64::from(size_mode.width) / in_width;
            let zoom_y = f64::from(size_mode.height) / in_height;

            let zoom = zoom_x.min(zoom_y);

            out_width = zoom * in_width;
            out_height = zoom * in_height;
        }

        SizeKind::WidthHeight => {
            if size_mode.width != -1 {
                out_width = f64::from(size_mode.width);
            } else {
                out_width = in_width;
            }

            if size_mode.height != -1 {
                out_height = f64::from(size_mode.height);
            } else {
                out_height = in_height;
            }
        }
    }

    (out_width, out_height)
}

fn render_to_pixbuf_at_size(
    handle: &Handle,
    document_width: f64,
    document_height: f64,
    desired_width: f64,
    desired_height: f64,
    dpi: Dpi,
) -> Result<Pixbuf, RenderingError> {
    if desired_width == 0.0
        || desired_height == 0.0
        || document_width == 0.0
        || document_height == 0.0
    {
        return empty_pixbuf();
    }

    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        checked_i32(desired_width.round())?,
        checked_i32(desired_height.round())?,
    )?;

    {
        let cr = cairo::Context::new(&surface);
        cr.scale(
            desired_width / document_width,
            desired_height / document_height,
        );

        let viewport = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: document_width,
            height: document_height,
        };

        // We do it with a cr transform so we can scale non-proportionally.
        handle.render_document(&cr, &viewport, dpi, false)?;
    }

    let shared_surface = SharedImageSurface::wrap(surface, SurfaceType::SRgb)?;

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
    Ok(Url::parse(&file.get_uri()).map_err(|_| LoadingError::BadUrl)?)
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
            .map_err(LoadingError::from)
            .and_then(|stream| Handle::from_stream(&load_options, stream.as_ref(), None))
        {
            Ok(handle) => handle,
            Err(e) => {
                set_gerror(error, 0, &format!("{}", e));
                return ptr::null_mut();
            }
        };

        handle
            .get_geometry_sub(None, dpi, false)
            .and_then(|(ink_r, _)| {
                let (document_width, document_height) = (ink_r.width(), ink_r.height());
                let (desired_width, desired_height) =
                    get_final_size(document_width, document_height, size_mode);

                render_to_pixbuf_at_size(
                    &handle,
                    document_width,
                    document_height,
                    desired_width,
                    desired_height,
                    dpi,
                )
            })
            .map(|pixbuf| pixbuf.to_glib_full())
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
    rsvg_return_val_if_fail! {
        rsvg_pixbuf_from_file_at_size => ptr::null_mut();

        !filename.is_null(),
        (width >= 1 && height >= 1) || (width == -1 && height == -1),
        error.is_null() || (*error).is_null(),
    }

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
    rsvg_return_val_if_fail! {
        rsvg_pixbuf_from_file_at_zoom => ptr::null_mut();

        !filename.is_null(),
        x_zoom > 0.0 && y_zoom > 0.0,
        error.is_null() || (*error).is_null(),
    }

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
    max_width: i32,
    max_height: i32,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    rsvg_return_val_if_fail! {
        rsvg_pixbuf_from_file_at_zoom_with_max => ptr::null_mut();

        !filename.is_null(),
        x_zoom > 0.0 && y_zoom > 0.0,
        max_width >= 1 && max_height >= 1,
        error.is_null() || (*error).is_null(),
    }

    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::ZoomMax,
            x_zoom,
            y_zoom,
            width: max_width,
            height: max_height,
        },
        error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_pixbuf_from_file_at_max_size(
    filename: *const libc::c_char,
    max_width: i32,
    max_height: i32,
    error: *mut *mut glib_sys::GError,
) -> *mut gdk_pixbuf_sys::GdkPixbuf {
    rsvg_return_val_if_fail! {
        rsvg_pixbuf_from_file_at_max_size => ptr::null_mut();

        !filename.is_null(),
        max_width >= 1 && max_height >= 1,
        error.is_null() || (*error).is_null(),
    }

    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::WidthHeightMax,
            x_zoom: 0.0,
            y_zoom: 0.0,
            width: max_width,
            height: max_height,
        },
        error,
    )
}
