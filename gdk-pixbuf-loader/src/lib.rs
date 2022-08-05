use std::ptr::null_mut;

use gdk_pixbuf_sys::{
    GdkPixbufFormat, GdkPixbufModule, GdkPixbufModulePattern, GdkPixbufModulePreparedFunc,
    GdkPixbufModuleSizeFunc, GdkPixbufModuleUpdatedFunc, GDK_PIXBUF_FORMAT_SCALABLE,
    GDK_PIXBUF_FORMAT_THREADSAFE,
};

use glib_sys::GError;
use gobject_sys::GObject;
use libc::{c_char, c_int, c_uint, c_void};

use gio::prelude::MemoryInputStreamExt;
use gio::MemoryInputStream;
use glib::Bytes;
use librsvg::Loader;

#[allow(non_camel_case_types)]
type c_bool = c_int;

use cstr::cstr;

struct SvgContext {
    size_func: GdkPixbufModuleSizeFunc,
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: *mut c_void,
    stream: MemoryInputStream,
}

#[no_mangle]
unsafe extern "C" fn begin_load(
    size_func: GdkPixbufModuleSizeFunc,
    prep_func: GdkPixbufModulePreparedFunc,
    update_func: GdkPixbufModuleUpdatedFunc,
    user_data: *mut c_void,
    error: *mut *mut GError,
) -> *mut c_void {
    if error != null_mut() {
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

    Box::into_raw(ctx) as *mut c_void
}

#[no_mangle]
unsafe extern "C" fn load_increment(
    user_data: *mut c_void,
    buffer: *const u8,
    size: c_uint,
    error: *mut *mut GError,
) -> c_bool {
    if error != null_mut() {
        *error = null_mut();
    }

    let ctx = user_data as *mut SvgContext;

    let data = std::slice::from_raw_parts(buffer, size as usize);
    let data = data.to_vec();
    (&*ctx).stream.add_bytes(&Bytes::from_owned(data));
    1
}

fn argb_to_rgba(data: &mut Vec<u8>, width: usize, height: usize, stride: usize) {
    assert!((width * 4) >= stride);
    assert!((stride * height) <= data.len());
    for i in 0..height {
        let row_index = i * stride;
        for j in 0..width {
            let pixel_index = row_index + (j * 4);
            let tmp = data[pixel_index + 2];
            data[pixel_index + 2] = data[pixel_index];
            data[pixel_index] = tmp;
        }
    }
}

#[no_mangle]
unsafe extern "C" fn stop_load(user_data: *mut c_void, error: *mut *mut GError) -> c_int {
    let ctx = Box::from_raw(user_data as *mut SvgContext);
    if error != null_mut() {
        *error = null_mut();
    }

    fn _inner_stop_load(ctx: &Box<SvgContext>) -> Result<(Vec<u8>, i32, i32, i32), String> {
        let handle = Loader::new()
            .read_stream::<_, gio::File, gio::Cancellable>(&ctx.stream, None, None)
            .map_err(|e| e.to_string())?;

        let renderer = librsvg::CairoRenderer::new(&handle);
        let (w, h) = match renderer.intrinsic_size_in_pixels() {
            Some((w, h)) => (w, h),
            None => {
                return Err(String::from(
                    "Could not get intrinsic size in pixel of Cairo memory surface",
                ));
            }
        };

        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, w.ceil() as i32, h.ceil() as i32)
                .map_err(|e| e.to_string())?;

        let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;

        renderer
            .render_document(
                &cr,
                &cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: w,
                    height: h,
                },
            )
            .map_err(|e| e.to_string())?;

        let w = w.ceil() as i32;
        let h = h.ceil() as i32;

        let stride = surface.stride();
        // The cairo::Context holds a reference to the surface which needs to be dropped to access the data
        std::mem::drop(cr);
        let sfc_data = surface.take_data().map_err(|e| e.to_string())?;
        let sfc_data = unsafe { std::slice::from_raw_parts(sfc_data.as_ptr(), sfc_data.len()) }; // This is just a slice to the canonical data
                                                                                                 // We need it as a mutable vector to move the alpha channel around
        let pb_data = sfc_data.to_vec();

        Ok((pb_data, w, h, stride))
    }

    let (mut pb_data, mut w, mut h, stride) = match _inner_stop_load(&ctx) {
        Ok(r) => r,
        Err(e) => {
            let gerr = glib::Error::new(gdk_pixbuf::PixbufError::Failed, &e.to_string());
            *error = gerr.into_raw();
            return 0;
        }
    };

    // GDK Pixbuf only support RGBA and Cairo only ARGB32, we swap channels around
    argb_to_rgba(&mut pb_data, w as usize, h as usize, stride as usize);

    // Vector length and capacity to rebuild and destroy the vector in destroy_fn
    let cb_data = Box::new((pb_data.len(), pb_data.capacity()));

    // Function to free the pixel data by rebuilding the Vec object
    #[no_mangle]
    unsafe extern "C" fn destroy_cb_foo(pixels: *mut u8, user_data: *mut c_void) {
        let data = Box::<(usize, usize)>::from_raw(user_data as *mut (usize, usize));
        Vec::from_raw_parts(pixels, data.0, data.1);
    }

    let pb_data = pb_data.leak::<'static>(); // Allocator stops tracking vector data
    let pixbuf = gdk_pixbuf_sys::gdk_pixbuf_new_from_data(
        pb_data.as_mut_ptr(),
        gdk_pixbuf_sys::GDK_COLORSPACE_RGB,
        1,
        8,
        w,
        h,
        stride,
        Some(destroy_cb_foo),
        Box::into_raw(cb_data) as *mut c_void,
    );
    if let Some(size_func) = ctx.size_func {
        size_func(&mut w as *mut i32, &mut h as *mut i32, ctx.user_data);
    }
    if let Some(update_func) = ctx.update_func {
        update_func(pixbuf, 0, 0, w, h, ctx.user_data);
    }
    if let Some(prep_func) = ctx.prep_func {
        prep_func(pixbuf, null_mut(), ctx.user_data);
    }

    // The module loader increases a ref so we drop the pixbuf here
    gobject_sys::g_object_unref(pixbuf as *mut GObject);

    1
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

const MIME_TYPES: [*const i8; 7] = [
    cstr!("image/svg+xml").as_ptr(),
    cstr!("image/svg").as_ptr(),
    cstr!("image/svg-xml").as_ptr(),
    cstr!("image/vnd.adobe.svg+xml").as_ptr(),
    cstr!("text/xml-svg").as_ptr(),
    cstr!("image/svg+xml-compressed").as_ptr(),
    std::ptr::null(),
];

const EXTENSIONS: [*const i8; 4] = [
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

    use crate::{EXTENSIONS, MIME_TYPES};
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
            disabled: 0,
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

    fn check_null_terminated_arr_cstrings(arr: &[*const i8]) {
        let n_strings = arr
            .iter()
            .filter(|e| e != &&null())
            .map(|e| {
                if e != &null() {
                    // We use strlen in all of them to ensure some safety
                    // We could use CStr instead but it'd be a bit more cumbersome
                    assert!(unsafe { libc::strlen(*e as *const i8) } > 0)
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
                    assert_eq!(unsafe { libc::strlen((*ptr).prefix as *mut i8) }, unsafe {
                        libc::strlen((*ptr).mask as *mut i8)
                    });
                }
                // Relevance must be 0 to 100
                assert!(unsafe { (*ptr).relevance } >= 0);
                assert!(unsafe { (*ptr).relevance } <= 100);
            }
        }
    }

    const SVG_DATA: &'static str = r#"<svg
            width="100" height="100" viewBox="0 0 26.458333 26.458333" version="1.1" id="svg5" xmlns="http://www.w3.org/2000/svg"
            xmlns:svg="http://www.w3.org/2000/svg">
            <defs id="defs2" />
            <g id="layer1">
                <rect style="fill:#aa1144;stroke-width:0.130147" id="rect31"
                    width="26.458334" height="26.458334"
                    x="-3.1789145e-07" y="-3.1789145e-07" />
            </g>
        </svg>"#;

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
            assert_eq!(h, 100);

            let pixels = gdk_pixbuf_sys::gdk_pixbuf_get_pixels(pb);

            // Upper left pixel, black with full opacity #000000ff
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

        let ctx = unsafe { crate::begin_load(None, Some(prep_cb), None, null_mut(), null_mut()) };
        assert_ne!(ctx, null_mut());

        let inc = unsafe {
            crate::load_increment(ctx, SVG_DATA.as_ptr(), SVG_DATA.len() as u32, null_mut())
        };
        assert_ne!(inc, 0);

        unsafe { crate::stop_load(ctx, null_mut()) };
    }
}
