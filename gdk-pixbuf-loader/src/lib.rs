use std::ptr::null_mut;

use gdk_pixbuf::ffi::{
    GdkPixbuf, GdkPixbufFormat, GdkPixbufModule, GdkPixbufModulePattern,
    GdkPixbufModulePreparedFunc, GdkPixbufModuleSizeFunc, GdkPixbufModuleUpdatedFunc,
    GDK_PIXBUF_FORMAT_SCALABLE, GDK_PIXBUF_FORMAT_THREADSAFE,
};

use libc::{c_char, c_int, c_uint};

use glib::ffi::{gboolean, gpointer, GError};
use glib::translate::*;
use glib::Bytes;

use gio::prelude::MemoryInputStreamExt;
use gio::MemoryInputStream;
use glib::gobject_ffi::GObject;

use librsvg_c::sizing::LegacySize;
use rsvg::Loader;

use cstr::cstr;

struct SvgContext {
    size_func: GdkPixbufModuleSizeFunc,
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: gpointer,
    stream: MemoryInputStream,
}

#[no_mangle]
unsafe extern "C" fn begin_load(
    size_func: GdkPixbufModuleSizeFunc,
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: gpointer,
    error: *mut *mut GError,
) -> gpointer {
    if !error.is_null() {
        *error = null_mut();
    }

    let stream = MemoryInputStream::new();
    let ctx = Box::new(SvgContext {
        size_func,
        prep_func,
        update_func,
        user_data,
        stream,
    });

    Box::into_raw(ctx) as gpointer
}

#[no_mangle]
unsafe extern "C" fn load_increment(
    user_data: gpointer,
    buffer: *const u8,
    size: c_uint,
    error: *mut *mut GError,
) -> gboolean {
    if !error.is_null() {
        *error = null_mut();
    }

    let ctx = user_data as *mut SvgContext;

    let data = std::slice::from_raw_parts(buffer, size as usize);
    (*ctx).stream.add_bytes(&Bytes::from(data));
    true.into_glib()
}

#[no_mangle]
unsafe extern "C" fn stop_load(user_data: gpointer, error: *mut *mut GError) -> gboolean {
    let ctx = Box::from_raw(user_data as *mut SvgContext);
    if !error.is_null() {
        *error = null_mut();
    }

    fn _inner_stop_load(ctx: &SvgContext) -> Result<gdk_pixbuf::Pixbuf, String> {
        let handle = Loader::new()
            .read_stream::<_, gio::File, gio::Cancellable>(&ctx.stream, None, None)
            .map_err(|e| e.to_string())?;

        let renderer = rsvg::CairoRenderer::new(&handle);
        let (w, h) = renderer.legacy_document_size().map_err(|e| e.to_string())?;
        let mut w = w.ceil() as c_int;
        let mut h = h.ceil() as c_int;

        if let Some(size_func) = ctx.size_func {
            let mut tmp_w: c_int = w;
            let mut tmp_h: c_int = h;
            unsafe {
                size_func(
                    &mut tmp_w as *mut c_int,
                    &mut tmp_h as *mut c_int,
                    ctx.user_data,
                )
            };
            if tmp_w != 0 && tmp_h != 0 {
                w = tmp_w;
                h = tmp_h;
            }
        }

        let pb = librsvg_c::pixbuf_utils::render_to_pixbuf_at_size(
            &renderer, w as f64, h as f64, w as f64, h as f64,
        )
        .map_err(|e| e.to_string())?;

        Ok(pb)
    }

    let pixbuf = match _inner_stop_load(&ctx) {
        Ok(r) => r,
        Err(e) => {
            if !error.is_null() {
                let gerr = glib::Error::new(gdk_pixbuf::PixbufError::Failed, &e);
                *error = gerr.into_glib_ptr();
            }
            return false.into_glib();
        }
    };

    let w = pixbuf.width();
    let h = pixbuf.height();
    let pixbuf: *mut GdkPixbuf = pixbuf.to_glib_full();

    if let Some(prep_func) = ctx.prep_func {
        prep_func(pixbuf, null_mut(), ctx.user_data);
    }
    if let Some(update_func) = ctx.update_func {
        update_func(pixbuf, 0, 0, w, h, ctx.user_data);
    }

    // The module loader increases a ref so we drop the pixbuf here
    glib::gobject_ffi::g_object_unref(pixbuf as *mut GObject);

    true.into_glib()
}

#[no_mangle]
extern "C" fn fill_vtable(module: &mut GdkPixbufModule) {
    module.begin_load = Some(begin_load);
    module.stop_load = Some(stop_load);
    module.load_increment = Some(load_increment);
}

const SIGNATURE: [GdkPixbufModulePattern; 3] = [
    GdkPixbufModulePattern {
        prefix: cstr!(" <svg").as_ptr() as *mut c_char,
        mask: cstr!("*    ").as_ptr() as *mut c_char,
        relevance: 100,
    },
    GdkPixbufModulePattern {
        prefix: cstr!(" <!DOCTYPE svg").as_ptr() as *mut c_char,
        mask: cstr!("*             ").as_ptr() as *mut c_char,
        relevance: 100,
    },
    GdkPixbufModulePattern {
        prefix: std::ptr::null_mut(),
        mask: std::ptr::null_mut(),
        relevance: 0,
    },
];

const MIME_TYPES: [*const c_char; 7] = [
    cstr!("image/svg+xml").as_ptr(),
    cstr!("image/svg").as_ptr(),
    cstr!("image/svg-xml").as_ptr(),
    cstr!("image/vnd.adobe.svg+xml").as_ptr(),
    cstr!("text/xml-svg").as_ptr(),
    cstr!("image/svg+xml-compressed").as_ptr(),
    std::ptr::null(),
];

const EXTENSIONS: [*const c_char; 4] = [
    cstr!("svg").as_ptr(),
    cstr!("svgz").as_ptr(),
    cstr!("svg.gz").as_ptr(),
    std::ptr::null(),
];

#[no_mangle]
extern "C" fn fill_info(info: &mut GdkPixbufFormat) {
    info.name = cstr!("svg").as_ptr() as *mut c_char;
    info.signature = SIGNATURE.as_ptr() as *mut GdkPixbufModulePattern;
    info.description = cstr!("Scalable Vector Graphics").as_ptr() as *mut c_char; //TODO: Gettext this
    info.mime_types = MIME_TYPES.as_ptr() as *mut *mut c_char;
    info.extensions = EXTENSIONS.as_ptr() as *mut *mut c_char;
    info.flags = GDK_PIXBUF_FORMAT_SCALABLE | GDK_PIXBUF_FORMAT_THREADSAFE;
    info.license = cstr!("LGPL").as_ptr() as *mut c_char;
}

#[cfg(test)]
mod tests {
    use gdk_pixbuf::ffi::{
        GdkPixbufFormat, GDK_PIXBUF_FORMAT_SCALABLE, GDK_PIXBUF_FORMAT_THREADSAFE,
    };
    use glib::translate::IntoGlib;

    use crate::{EXTENSIONS, MIME_TYPES};
    use libc::c_char;
    use std::ptr::null_mut;

    fn pb_format_new() -> GdkPixbufFormat {
        let mut info = super::GdkPixbufFormat {
            name: null_mut(),
            signature: null_mut(),
            description: null_mut(),
            mime_types: null_mut(),
            extensions: null_mut(),
            flags: 0,
            license: null_mut(),
            disabled: false.into_glib(),
            domain: null_mut(),
        };

        super::fill_info(&mut info);
        info
    }

    #[test]
    fn fill_info() {
        let info = pb_format_new();

        assert_ne!(info.name, null_mut());
        assert_ne!(info.signature, null_mut());
        assert_ne!(info.description, null_mut());
        assert_ne!(info.mime_types, null_mut());
        assert_ne!(info.extensions, null_mut());
        assert_eq!(
            info.flags,
            GDK_PIXBUF_FORMAT_SCALABLE | GDK_PIXBUF_FORMAT_THREADSAFE
        );
        assert_ne!(info.license, null_mut());
    }

    fn check_null_terminated_arr_cstrings(arr: &[*const c_char]) {
        let n_strings = arr
            .iter()
            .filter(|e| !e.is_null())
            .map(|e| {
                if !e.is_null() {
                    // We use strlen in all of them to ensure some safety
                    // We could use CStr instead but it'd be a bit more cumbersome
                    assert!(unsafe { libc::strlen(*e as *const c_char) } > 0)
                }
            })
            .count(); // Count all non_null items

        // Ensure last item is null and is the only null item
        assert_eq!(n_strings, arr.len() - 1);
        assert!(arr.last().unwrap().is_null());
    }

    #[test]
    fn extensions_bounds() {
        check_null_terminated_arr_cstrings(&EXTENSIONS);
    }

    #[test]
    fn mime_bounds() {
        check_null_terminated_arr_cstrings(&MIME_TYPES)
    }

    #[test]
    fn signature() {
        let info = pb_format_new();
        unsafe {
            for i in 0..2 {
                let ptr = info.signature.offset(i);
                if i == 2 {
                    assert!((*ptr).prefix.is_null());
                    continue;
                } else {
                    assert!(!(*ptr).prefix.is_null());
                    if (*ptr).mask != null_mut() {
                        // Mask can be null
                        assert_eq!(
                            libc::strlen((*ptr).prefix as *mut c_char),
                            libc::strlen((*ptr).mask as *mut c_char)
                        );
                    }
                    // Relevance must be 0 to 100
                    assert!((*ptr).relevance >= 0);
                    assert!((*ptr).relevance <= 100);
                }
            }
        }
    }

    const SVG_DATA: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
                                    <svg
                                    width="100px"
                                    height="150px"
                                    viewBox="0 0 26.458333 26.458333"
                                    version="1.1"
                                    id="svg5"
                                    xmlns="http://www.w3.org/2000/svg"
                                    xmlns:svg="http://www.w3.org/2000/svg">
                                    <rect
                                        style="fill:#aa1144;stroke-width:0.0344347"
                                        width="26.458332"
                                        height="39.6875"
                                        x="4.691162e-07"
                                        y="-6.6145835"
                                        id="rect2" />
                                    </svg>
    "#;

    #[test]
    fn minimal_svg() {
        unsafe extern "C" fn prep_cb(
            pb: *mut gdk_pixbuf::ffi::GdkPixbuf,
            pba: *mut gdk_pixbuf::ffi::GdkPixbufAnimation,
            user_data: *mut libc::c_void,
        ) {
            assert_eq!(user_data, null_mut());
            assert_eq!(pba, null_mut());

            let w = gdk_pixbuf::ffi::gdk_pixbuf_get_width(pb);
            let h = gdk_pixbuf::ffi::gdk_pixbuf_get_height(pb);
            let stride = gdk_pixbuf::ffi::gdk_pixbuf_get_rowstride(pb);
            assert_eq!(w, 100);
            assert_eq!(h, 150);

            let pixels = gdk_pixbuf::ffi::gdk_pixbuf_get_pixels(pb);

            // Upper left pixel #aa1144ff
            assert_eq!(*pixels, 0xaa);
            assert_eq!(*pixels.offset(1), 0x11);
            assert_eq!(*pixels.offset(2), 0x44);
            assert_eq!(*pixels.offset(3), 0xff);

            // Bottom left pixel
            assert_eq!(*pixels.offset((stride * (h - 1)) as isize), 0xaa);
            assert_eq!(*pixels.offset((stride * (h - 1)) as isize + 1), 0x11);
            assert_eq!(*pixels.offset((stride * (h - 1)) as isize + 2), 0x44);
            assert_eq!(*pixels.offset((stride * (h - 1)) as isize + 3), 0xff);

            // Bottom right pixel
            assert_eq!(
                *pixels.offset((stride * (h - 1)) as isize + (w as isize - 1) * 4),
                0xaa
            );
            assert_eq!(
                *pixels.offset((stride * (h - 1)) as isize + (w as isize - 1) * 4 + 1),
                0x11
            );
            assert_eq!(
                *pixels.offset((stride * (h - 1)) as isize + (w as isize - 1) * 4 + 2),
                0x44
            );
            assert_eq!(
                *pixels.offset((stride * (h - 1)) as isize + (w as isize - 1) * 4 + 3),
                0xff
            );
        }
        unsafe {
            let ctx = crate::begin_load(None, Some(prep_cb), None, null_mut(), null_mut());
            assert_ne!(ctx, null_mut());

            let inc =
                crate::load_increment(ctx, SVG_DATA.as_ptr(), SVG_DATA.len() as u32, null_mut());
            assert_ne!(inc, 0);

            crate::stop_load(ctx, null_mut());
        }
    }
}
