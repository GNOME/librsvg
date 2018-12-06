use std::ptr;

use cairo::{ImageSurface, Status};
use cairo_sys;
use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use glib;
use glib::translate::*;
use glib_sys;
use libc;

use css::CssStyles;
use defs::Defs;
use error::LoadingError;
use surface_utils::shared_surface::SharedImageSurface;

#[repr(C)]
pub struct RsvgHandle {
    _private: [u8; 0],
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_handle_get_defs(handle: *const RsvgHandle) -> *const Defs;

    fn rsvg_handle_resolve_uri(
        handle: *const RsvgHandle,
        uri: *const libc::c_char,
    ) -> *const libc::c_char;

    fn rsvg_handle_load_extern(
        handle: *const RsvgHandle,
        uri: *const libc::c_char,
    ) -> *const RsvgHandle;

    fn rsvg_handle_get_css_styles(handle: *const RsvgHandle) -> *mut CssStyles;

    fn _rsvg_handle_acquire_data(
        handle: *mut RsvgHandle,
        url: *const libc::c_char,
        out_content_type: *mut *mut libc::c_char,
        out_len: *mut usize,
        error: *mut *mut glib_sys::GError,
    ) -> *mut u8;

    fn rsvg_handle_keep_image_data(handle: *const RsvgHandle) -> glib_sys::gboolean;
}

pub fn get_defs<'a>(handle: *const RsvgHandle) -> &'a Defs {
    unsafe {
        let d = rsvg_handle_get_defs(handle);
        &*d
    }
}

pub fn resolve_uri(handle: *const RsvgHandle, uri: &str) -> Option<String> {
    unsafe {
        let resolved = rsvg_handle_resolve_uri(handle, uri.to_glib_none().0);
        if resolved.is_null() {
            None
        } else {
            Some(from_glib_full(resolved))
        }
    }
}

pub fn load_extern(handle: *const RsvgHandle, uri: &str) -> *const RsvgHandle {
    unsafe { rsvg_handle_load_extern(handle, uri.to_glib_none().0) }
}

pub fn get_css_styles<'a>(handle: *const RsvgHandle) -> &'a CssStyles {
    unsafe { &*rsvg_handle_get_css_styles(handle) }
}

pub fn get_css_styles_mut<'a>(handle: *const RsvgHandle) -> &'a mut CssStyles {
    unsafe { &mut *rsvg_handle_get_css_styles(handle) }
}

pub struct BinaryData {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn acquire_data(handle: *mut RsvgHandle, url: &str) -> Result<BinaryData, glib::Error> {
    unsafe {
        let mut content_type: *mut libc::c_char = ptr::null_mut();
        let mut len = 0;
        let mut error = ptr::null_mut();

        let buf = _rsvg_handle_acquire_data(
            handle,
            url.to_glib_none().0,
            &mut content_type as *mut *mut _,
            &mut len,
            &mut error,
        );

        if buf.is_null() {
            if error.is_null() && len == 0 {
                Ok(BinaryData {
                    data: Vec::new(),
                    content_type: None,
                })
            } else {
                Err(from_glib_full(error))
            }
        } else {
            Ok(BinaryData {
                data: FromGlibContainer::from_glib_full_num(buf as *mut u8, len),
                content_type: from_glib_full(content_type),
            })
        }
    }
}

fn keep_image_data(handle: *const RsvgHandle) -> bool {
    unsafe { from_glib(rsvg_handle_keep_image_data(handle)) }
}

pub fn image_surface_new_from_href(
    handle: *mut RsvgHandle,
    href: &str,
) -> Result<ImageSurface, LoadingError> {
    let data = acquire_data(handle, href)?;

    if data.data.len() == 0 {
        return Err(LoadingError::EmptyData);
    }

    let loader = if let Some(ref content_type) = data.content_type {
        PixbufLoader::new_with_mime_type(content_type)?
    } else {
        PixbufLoader::new()
    };

    loader.write(&data.data)?;
    loader.close()?;

    let pixbuf = loader.get_pixbuf().ok_or(LoadingError::Unknown)?;

    let surface = SharedImageSurface::from_pixbuf(&pixbuf)?.into_image_surface()?;

    if keep_image_data(handle) {
        let mime_type = data.content_type.or_else(|| {
            // Try to get the content type from the loader

            loader.get_format().and_then(|format| {
                let content_types = format.get_mime_types();

                if content_types.len() != 0 {
                    Some(content_types[0].clone())
                } else {
                    None
                }
            })
        });

        if let Some(mime_type) = mime_type {
            extern "C" {
                fn cairo_surface_set_mime_data(
                    surface: *mut cairo_sys::cairo_surface_t,
                    mime_type: *const libc::c_char,
                    data: *mut libc::c_char,
                    length: libc::c_ulong,
                    destroy: cairo_sys::cairo_destroy_func_t,
                    closure: *mut libc::c_void,
                ) -> Status;
            }

            let data_ptr = ToGlibContainerFromSlice::to_glib_full_from_slice(&data.data);

            unsafe {
                let status = cairo_surface_set_mime_data(
                    surface.to_glib_none().0,
                    mime_type.to_glib_none().0,
                    data_ptr as *mut _,
                    data.data.len() as libc::c_ulong,
                    Some(glib_sys::g_free),
                    data_ptr as *mut _,
                );

                if status != Status::Success {
                    return Err(LoadingError::Cairo(status));
                }
            }
        }
    }

    Ok(surface)
}
