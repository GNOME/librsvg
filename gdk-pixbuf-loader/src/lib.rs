use std::ptr::{null_mut, null};

use gdk_pixbuf_sys::{
    GdkPixbufFormat, GdkPixbufModule, GdkPixbufModulePattern, GdkPixbufModulePreparedFunc,
    GdkPixbufModuleSizeFunc, GdkPixbufModuleUpdatedFunc, GDK_PIXBUF_FORMAT_SCALABLE,
    GDK_PIXBUF_FORMAT_THREADSAFE,
};

use libc::{c_char, c_uint, size_t, c_int};

use glib::translate::IntoGlib;
use glib_sys::{gboolean, GError};

use gobject_sys::GObject;

use cstr::cstr;

#[repr(C)]
struct RsvgHandle {
    _dumb: c_char
}

#[link(name = "rsvg-2")]
extern "C" {
    fn rsvg_handle_new() -> *const RsvgHandle;
    fn rsvg_handle_write(
        handle: *const RsvgHandle,
        buf: *const u8,
        count: usize,
        error: *mut *mut glib_sys::GError,
    ) -> glib_sys::gboolean;
    fn rsvg_handle_close(
        handle: *const RsvgHandle,
        error: *mut *mut glib_sys::GError,
    ) -> glib_sys::gboolean;

    fn rsvg_handle_get_pixbuf(handle: *const RsvgHandle) -> *mut gdk_pixbuf_sys::GdkPixbuf;

    fn rsvg_handle_set_size_callback(
        handle: *const RsvgHandle,
        size_func: GdkPixbufModuleSizeFunc,
        user_data: glib_sys::gpointer,
        destroy_notify: glib_sys::GDestroyNotify,
    );

    fn rsvg_handle_free(handle: *mut RsvgHandle);
}

struct SvgContext {
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: glib_sys::gpointer,
    handle: *const RsvgHandle,
}

impl Drop for SvgContext {
    fn drop(&mut self) {
        if self.handle != null() {
            unsafe { rsvg_handle_free(self.handle as *mut RsvgHandle) };
        }
    }
}

#[no_mangle]
unsafe extern "C" fn begin_load(
    size_func: GdkPixbufModuleSizeFunc,
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: glib_sys::gpointer,
    error: *mut *mut GError,
) -> glib_sys::gpointer {
    if error != null_mut() {
        *error = null_mut();
    }

    let handle = rsvg_handle_new();
    if handle == null() {
        return null_mut();
    }

    rsvg_handle_set_size_callback(handle, size_func, user_data, None);
    let ctx = Box::new(SvgContext {
        prep_func,
        update_func,
        user_data,
        handle,
    });

    Box::into_raw(ctx) as glib_sys::gpointer
}

#[no_mangle]
unsafe extern "C" fn load_increment(
    user_data: glib_sys::gpointer,
    buffer: *const u8,
    size: c_uint,
    error: *mut *mut GError,
) -> gboolean {
    if error != null_mut() {
        *error = null_mut();
    }

    let ctx = user_data as *mut SvgContext;
    rsvg_handle_write((*ctx).handle, buffer, size as size_t, error)
}

#[no_mangle]
unsafe extern "C" fn stop_load(user_data: glib_sys::gpointer, error: *mut *mut GError) -> gboolean {
    let ctx = Box::from_raw(user_data as *mut SvgContext);
    if error != null_mut() {
        *error = null_mut();
    }

    if rsvg_handle_close(ctx.handle, error) == false.into_glib() {
        return false.into_glib();
    }

    let pixbuf = rsvg_handle_get_pixbuf((*ctx).handle);

    if let Some(prep_func) = ctx.prep_func {
        prep_func(pixbuf, null_mut(), ctx.user_data);
    }
    if let Some(update_func) = ctx.update_func {
        let w: c_int = gdk_pixbuf_sys::gdk_pixbuf_get_width(pixbuf);
        let h: c_int = gdk_pixbuf_sys::gdk_pixbuf_get_height(pixbuf);
        update_func(pixbuf, 0, 0, w, h, ctx.user_data);
    }

    // The module loader increases a ref so we drop the pixbuf here
    gobject_sys::g_object_unref(pixbuf as *mut GObject);

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
    use gdk_pixbuf_sys::{
        GdkPixbufFormat, GDK_PIXBUF_FORMAT_SCALABLE, GDK_PIXBUF_FORMAT_THREADSAFE,
    };
    use glib::translate::IntoGlib;

    use crate::{EXTENSIONS, MIME_TYPES};
    use libc::c_char;
    use std::ptr::{null, null_mut};

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
            .filter(|e| e != &&null())
            .map(|e| {
                if e != &null() {
                    // We use strlen in all of them to ensure some safety
                    // We could use CStr instead but it'd be a bit more cumbersome
                    assert!(unsafe { libc::strlen(*e as *const c_char) } > 0)
                }
            })
            .count(); // Count all non_null items

        // Ensure last item is null and is the only null item
        assert_eq!(n_strings, arr.len() - 1);
        assert!(arr.last().unwrap() == &null());
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

        for i in 0..2 {
            let ptr = unsafe { info.signature.offset(i) };
            if i == 2 {
                assert_eq!(unsafe { (*ptr).prefix }, null_mut());
                continue;
            } else {
                assert_ne!(unsafe { (*ptr).prefix }, null_mut());
                if unsafe { (*ptr).mask } != null_mut() {
                    // Mask can be null
                    assert_eq!(
                        unsafe { libc::strlen((*ptr).prefix as *mut c_char) },
                        unsafe { libc::strlen((*ptr).mask as *mut c_char) }
                    );
                }
                // Relevance must be 0 to 100
                assert!(unsafe { (*ptr).relevance } >= 0);
                assert!(unsafe { (*ptr).relevance } <= 100);
            }
        }
    }

    const SVG_DATA: &'static str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
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

    const SVG_BROKEN: &'static str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
                                    <svg
                                    width="100px"
                                    height="150px"
                                    viewBox="0 0 26.458333 26.458333"
                                    version="1.1" BROKEN DATA"#;

    #[test]
    fn minimal_svg() {
        unsafe extern "C" fn prep_cb(
            pb: *mut gdk_pixbuf_sys::GdkPixbuf,
            pba: *mut gdk_pixbuf_sys::GdkPixbufAnimation,
            user_data: *mut libc::c_void,
        ) {
            assert_eq!(user_data, null_mut());
            assert_eq!(pba, null_mut());

            let w = gdk_pixbuf_sys::gdk_pixbuf_get_width(pb);
            let h = gdk_pixbuf_sys::gdk_pixbuf_get_height(pb);
            let stride = gdk_pixbuf_sys::gdk_pixbuf_get_rowstride(pb);
            assert_eq!(w, 100);
            assert_eq!(h, 150);

            let pixels = gdk_pixbuf_sys::gdk_pixbuf_get_pixels(pb);

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

        let mut err: *mut glib_sys::GError = null_mut();
        let err_arg = &mut err as *mut *mut glib_sys::GError;

        let ctx = unsafe { crate::begin_load(None, Some(prep_cb), None, null_mut(), err_arg) };
        assert_ne!(ctx, null_mut());

        let inc = unsafe {
            crate::load_increment(ctx, SVG_DATA.as_ptr(), SVG_DATA.len() as u32, err_arg)
        };
        assert_eq!(inc, true.into_glib());

        let ret = unsafe { crate::stop_load(ctx, err_arg) };
        assert_eq!(ret, true.into_glib());
    }

    #[test]
    fn broken_svg() {
        let mut err: *mut glib_sys::GError = null_mut();
        let err_arg = &mut err as *mut *mut glib_sys::GError;

        let ctx = unsafe { crate::begin_load(None, None, None, null_mut(), err_arg) };
        assert_ne!(ctx, null_mut());

        let ret = unsafe {
            crate::load_increment(ctx, SVG_BROKEN.as_ptr(), SVG_DATA.len() as u32, err_arg)
        };
        assert_eq!(ret, true.into_glib());

        let ret = unsafe { crate::stop_load(ctx, err_arg) };
        assert_eq!(ret, false.into_glib()); // should return false as it couldn't load SVG data
        assert!(err != null_mut()); // should set error

        unsafe { glib_sys::g_error_free(err) };
    }
}
