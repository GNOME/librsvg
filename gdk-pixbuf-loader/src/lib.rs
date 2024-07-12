use std::ptr::null_mut;

use gdk_pixbuf::ffi::{
    GdkPixbufFormat, GdkPixbufModule, GdkPixbufModulePattern, GdkPixbufModulePreparedFunc,
    GdkPixbufModuleSizeFunc, GdkPixbufModuleUpdatedFunc, GDK_PIXBUF_FORMAT_SCALABLE,
    GDK_PIXBUF_FORMAT_THREADSAFE,
};

use std::ffi::{c_char, c_int, c_uint};

use glib::ffi::{gboolean, gpointer, GDestroyNotify, GError};
use glib::prelude::*;
use glib::translate::*;
use glib::Bytes;

use gio::ffi::{GCancellable, GFile, GInputStream};
use gio::prelude::*;
use gio::MemoryInputStream;

use glib::gstr;

type RSvgHandle = glib::gobject_ffi::GObject;

type RsvgSizeFunc = Option<
    unsafe extern "C" fn(inout_width: *mut c_int, inout_height: *mut c_int, user_data: gpointer),
>;

#[link(name = "rsvg-2")]
extern "C" {
    fn rsvg_handle_new_from_stream_sync(
        input_stream: *mut GInputStream,
        base_file: *mut GFile,
        flags: u32,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
    ) -> *mut RSvgHandle;

    fn rsvg_handle_get_pixbuf_and_error(
        handle: *mut RSvgHandle,
        error: *mut *mut GError,
    ) -> *mut gdk_pixbuf::ffi::GdkPixbuf;

    fn rsvg_handle_set_size_callback(
        handle: *mut RSvgHandle,
        size_func: RsvgSizeFunc,
        user_data: gpointer,
        destroy_notify: GDestroyNotify,
    );
}

struct SvgContext {
    size_func: GdkPixbufModuleSizeFunc,
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: gpointer,
    stream: MemoryInputStream,
}

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

unsafe extern "C" fn stop_load(user_data: gpointer, error: *mut *mut GError) -> gboolean {
    let ctx = Box::from_raw(user_data as *mut SvgContext);
    if !error.is_null() {
        *error = null_mut();
    }

    let mut local_error = null_mut::<GError>();

    let handle = {
        let raw_handle = rsvg_handle_new_from_stream_sync(
            ctx.stream.upcast_ref::<gio::InputStream>().to_glib_none().0,
            null_mut(), // base_file
            0,
            null_mut(), // cancellable
            &mut local_error,
        );
        if !local_error.is_null() {
            if !error.is_null() {
                *error = local_error;
            }
            return false.into_glib();
        }

        glib::Object::from_glib_full(raw_handle)
    };

    rsvg_handle_set_size_callback(handle.as_ptr(), ctx.size_func, ctx.user_data, None);

    let pixbuf = {
        let p = rsvg_handle_get_pixbuf_and_error(handle.as_ptr(), &mut local_error);
        if !local_error.is_null() {
            if !error.is_null() {
                *error = local_error;
            }
            return false.into_glib();
        }

        gdk_pixbuf::Pixbuf::from_glib_full(p)
    };

    let w = pixbuf.width();
    let h = pixbuf.height();

    if let Some(prep_func) = ctx.prep_func {
        prep_func(pixbuf.to_glib_none().0, null_mut(), ctx.user_data);
    }
    if let Some(update_func) = ctx.update_func {
        update_func(pixbuf.to_glib_none().0, 0, 0, w, h, ctx.user_data);
    }

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
        prefix: gstr!(" <svg").as_ptr() as *mut c_char,
        mask: gstr!("*    ").as_ptr() as *mut c_char,
        relevance: 100,
    },
    GdkPixbufModulePattern {
        prefix: gstr!(" <!DOCTYPE svg").as_ptr() as *mut c_char,
        mask: gstr!("*             ").as_ptr() as *mut c_char,
        relevance: 100,
    },
    GdkPixbufModulePattern {
        prefix: null_mut(),
        mask: null_mut(),
        relevance: 0,
    },
];

const MIME_TYPES: [*const c_char; 7] = [
    gstr!("image/svg+xml").as_ptr(),
    gstr!("image/svg").as_ptr(),
    gstr!("image/svg-xml").as_ptr(),
    gstr!("image/vnd.adobe.svg+xml").as_ptr(),
    gstr!("text/xml-svg").as_ptr(),
    gstr!("image/svg+xml-compressed").as_ptr(),
    std::ptr::null(),
];

const EXTENSIONS: [*const c_char; 4] = [
    gstr!("svg").as_ptr(),
    gstr!("svgz").as_ptr(),
    gstr!("svg.gz").as_ptr(),
    std::ptr::null(),
];

#[no_mangle]
extern "C" fn fill_info(info: &mut GdkPixbufFormat) {
    info.name = gstr!("svg").as_ptr() as *mut c_char;
    info.signature = SIGNATURE.as_ptr() as *mut GdkPixbufModulePattern;
    info.description = gstr!("Scalable Vector Graphics").as_ptr() as *mut c_char; //TODO: Gettext this
    info.mime_types = MIME_TYPES.as_ptr() as *mut *mut c_char;
    info.extensions = EXTENSIONS.as_ptr() as *mut *mut c_char;
    info.flags = GDK_PIXBUF_FORMAT_SCALABLE | GDK_PIXBUF_FORMAT_THREADSAFE;
    info.license = gstr!("LGPL").as_ptr() as *mut c_char;
}

#[cfg(test)]
mod tests {
    use gdk_pixbuf::ffi::{
        GdkPixbufFormat, GDK_PIXBUF_FORMAT_SCALABLE, GDK_PIXBUF_FORMAT_THREADSAFE,
    };
    use glib::translate::IntoGlib;

    use crate::{EXTENSIONS, MIME_TYPES};
    use std::ffi::c_char;
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

        assert!(!info.name.is_null());
        assert!(!info.signature.is_null());
        assert!(!info.description.is_null());
        assert!(!info.mime_types.is_null());
        assert!(!info.extensions.is_null());
        assert_eq!(
            info.flags,
            GDK_PIXBUF_FORMAT_SCALABLE | GDK_PIXBUF_FORMAT_THREADSAFE
        );
        assert!(!info.license.is_null());
    }

    fn check_null_terminated_arr_cstrings(arr: &[*const c_char]) {
        let n_strings = arr
            .iter()
            .filter(|e| !e.is_null())
            .map(|e| {
                if !e.is_null() {
                    assert!(!unsafe { std::ffi::CStr::from_ptr(*e) }.is_empty())
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
                    if !(*ptr).mask.is_null() {
                        // Mask can be null
                        let prefix = std::ffi::CStr::from_ptr((*ptr).prefix).to_bytes();
                        let mask = std::ffi::CStr::from_ptr((*ptr).mask).to_bytes();
                        assert_eq!(prefix.len(), mask.len());
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
            user_data: *mut std::ffi::c_void,
        ) {
            assert!(user_data.is_null());
            assert!(pba.is_null());

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
