//! Legacy C API for functions that render directly to a `GdkPixbuf`.
//!
//! This is the implementation of the `rsvg_pixbuf_*` family of functions.

use std::path::PathBuf;
use std::ptr;

use gdk_pixbuf::{Colorspace, Pixbuf};
use glib::translate::*;

use super::dpi::Dpi;
use super::handle::{checked_i32, set_gerror};
use super::sizing::LegacySize;
use crate::api::{CairoRenderer, Loader};

use crate::error::RenderingError;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

pub fn empty_pixbuf() -> Result<Pixbuf, RenderingError> {
    // GdkPixbuf does not allow zero-sized pixbufs, but Cairo allows zero-sized
    // surfaces.  In this case, return a 1-pixel transparent pixbuf.

    let pixbuf = Pixbuf::new(Colorspace::Rgb, true, 8, 1, 1)
        .ok_or_else(|| RenderingError::OutOfMemory(String::from("creating a Pixbuf")))?;
    pixbuf.put_pixel(0, 0, 0, 0, 0, 0);

    Ok(pixbuf)
}

pub fn pixbuf_from_surface(surface: &SharedImageSurface) -> Result<Pixbuf, RenderingError> {
    surface
        .to_pixbuf()
        .ok_or_else(|| RenderingError::OutOfMemory(String::from("creating a Pixbuf")))
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
    renderer: &CairoRenderer<'_>,
    document_width: f64,
    document_height: f64,
    desired_width: f64,
    desired_height: f64,
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
        checked_i32(desired_width.ceil())?,
        checked_i32(desired_height.ceil())?,
    )?;

    {
        let cr = cairo::Context::new(&surface)?;
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
        renderer.render_document(&cr, &viewport)?;
    }

    let shared_surface = SharedImageSurface::wrap(surface, SurfaceType::SRgb)?;

    pixbuf_from_surface(&shared_surface)
}

unsafe fn pixbuf_from_file_with_size_mode(
    filename: *const libc::c_char,
    size_mode: &SizeMode,
    error: *mut *mut glib::ffi::GError,
) -> *mut gdk_pixbuf::ffi::GdkPixbuf {
    let path = PathBuf::from_glib_none(filename);

    let handle = match Loader::new().read_path(path) {
        Ok(handle) => handle,
        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            return ptr::null_mut();
        }
    };

    let dpi = Dpi::default();
    let renderer = CairoRenderer::new(&handle).with_dpi(dpi.x(), dpi.y());

    let (document_width, document_height) = match renderer.legacy_document_size() {
        Ok(dim) => dim,
        Err(e) => {
            set_gerror(error, 0, &format!("{}", e));
            return ptr::null_mut();
        }
    };

    let (desired_width, desired_height) =
        get_final_size(document_width, document_height, size_mode);

    render_to_pixbuf_at_size(
        &renderer,
        document_width,
        document_height,
        desired_width,
        desired_height,
    )
    .map(|pixbuf| pixbuf.to_glib_full())
    .unwrap_or_else(|e| {
        set_gerror(error, 0, &format!("{}", e));
        ptr::null_mut()
    })
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_pixbuf_from_file(
    filename: *const libc::c_char,
    error: *mut *mut glib::ffi::GError,
) -> *mut gdk_pixbuf::ffi::GdkPixbuf {
    rsvg_return_val_if_fail! {
        rsvg_pixbuf_from_file => ptr::null_mut();

        !filename.is_null(),
        error.is_null() || (*error).is_null(),
    }

    pixbuf_from_file_with_size_mode(
        filename,
        &SizeMode {
            kind: SizeKind::WidthHeight,
            x_zoom: 0.0,
            y_zoom: 0.0,
            width: -1,
            height: -1,
        },
        error,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_pixbuf_from_file_at_size(
    filename: *const libc::c_char,
    width: libc::c_int,
    height: libc::c_int,
    error: *mut *mut glib::ffi::GError,
) -> *mut gdk_pixbuf::ffi::GdkPixbuf {
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
pub unsafe extern "C" fn rsvg_pixbuf_from_file_at_zoom(
    filename: *const libc::c_char,
    x_zoom: libc::c_double,
    y_zoom: libc::c_double,
    error: *mut *mut glib::ffi::GError,
) -> *mut gdk_pixbuf::ffi::GdkPixbuf {
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
pub unsafe extern "C" fn rsvg_pixbuf_from_file_at_zoom_with_max(
    filename: *const libc::c_char,
    x_zoom: libc::c_double,
    y_zoom: libc::c_double,
    max_width: libc::c_int,
    max_height: libc::c_int,
    error: *mut *mut glib::ffi::GError,
) -> *mut gdk_pixbuf::ffi::GdkPixbuf {
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
pub unsafe extern "C" fn rsvg_pixbuf_from_file_at_max_size(
    filename: *const libc::c_char,
    max_width: libc::c_int,
    max_height: libc::c_int,
    error: *mut *mut glib::ffi::GError,
) -> *mut gdk_pixbuf::ffi::GdkPixbuf {
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
