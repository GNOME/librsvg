use data_url;
use gio_sys;
use glib_sys;
use libc;

use gio::{
    self,
    BufferedInputStream,
    BufferedInputStreamExt,
    Cancellable,
    ConverterInputStream,
    File as GFile,
    FileExt,
    InputStream,
    MemoryInputStream,
    ZlibCompressorFormat,
    ZlibDecompressor,
};
use glib::{self, translate::*, Bytes as GBytes, Cast};
use std::ptr;

use allowed_url::AllowedUrl;
use error::{set_gerror, LoadingError, RsvgError};
use handle::BinaryData;
use util::utf8_cstr;

fn decode_data_uri(uri: &str) -> Result<BinaryData, LoadingError> {
    let data_url = data_url::DataUrl::process(uri).map_err(|_| LoadingError::BadDataUrl)?;

    let mime_type = data_url.mime_type().to_string();

    let (bytes, fragment_id) = data_url
        .decode_to_vec()
        .map_err(|_| LoadingError::BadDataUrl)?;

    // See issue #377 - per the data: URL spec
    // (https://fetch.spec.whatwg.org/#data-urls), those URLs cannot
    // have fragment identifiers.  So, just return an error if we find
    // one.  This probably indicates mis-quoted SVG data inside the
    // data: URL.
    if fragment_id.is_some() {
        return Err(LoadingError::BadDataUrl);
    }

    Ok(BinaryData {
        data: bytes,
        content_type: Some(mime_type),
    })
}

pub fn binary_data_to_glib(
    binary_data: &BinaryData,
    out_mime_type: *mut *mut libc::c_char,
    out_size: *mut usize,
) -> *mut libc::c_char {
    unsafe {
        if !out_mime_type.is_null() {
            *out_mime_type = binary_data.content_type.to_glib_full();
        }

        *out_size = binary_data.data.len();

        ToGlibContainerFromSlice::to_glib_full_from_slice(&binary_data.data) as *mut libc::c_char
    }
}

#[no_mangle]
pub fn rsvg_decode_data_uri(
    uri: *const libc::c_char,
    out_mime_type: *mut *mut libc::c_char,
    out_size: *mut usize,
    error: *mut *mut glib_sys::GError,
) -> *mut libc::c_char {
    unsafe {
        assert!(!out_size.is_null());

        let uri = utf8_cstr(uri);

        match decode_data_uri(uri) {
            Ok(binary_data) => {
                if !error.is_null() {
                    *error = ptr::null_mut();
                }

                binary_data_to_glib(&binary_data, out_mime_type, out_size)
            }

            Err(_) => {
                if !out_mime_type.is_null() {
                    *out_mime_type = ptr::null_mut();
                }

                *out_size = 0;

                set_gerror(error, 0, "could not decode data: URL");

                ptr::null_mut()
            }
        }
    }
}

// Header of a gzip data stream
const GZ_MAGIC_0: u8 = 0x1f;
const GZ_MAGIC_1: u8 = 0x8b;

fn get_input_stream_for_loading(
    stream: InputStream,
    cancellable: Option<Cancellable>,
) -> Result<InputStream, glib::Error> {
    // detect gzipped streams (svgz)

    let buffered = BufferedInputStream::new(&stream);
    let num_read = buffered.fill(2, cancellable.as_ref())?;
    if num_read < 2 {
        // FIXME: this string was localized in the original; localize it
        return Err(glib::Error::new(RsvgError, "Input file is too short"));
    }

    let buf = buffered.peek_buffer();
    assert!(buf.len() >= 2);
    if buf[0] == GZ_MAGIC_0 && buf[1] == GZ_MAGIC_1 {
        let decomp = ZlibDecompressor::new(ZlibCompressorFormat::Gzip);
        let converter = ConverterInputStream::new(&buffered, &decomp);
        Ok(converter.upcast::<InputStream>())
    } else {
        Ok(buffered.upcast::<InputStream>())
    }
}

#[no_mangle]
pub unsafe fn rsvg_get_input_stream_for_loading(
    stream: *mut gio_sys::GInputStream,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> *mut gio_sys::GInputStream {
    let stream = from_glib_borrow(stream);
    let cancellable = from_glib_borrow(cancellable);

    match get_input_stream_for_loading(stream, cancellable) {
        Ok(stream) => stream.to_glib_full(),

        Err(e) => {
            if !error.is_null() {
                *error = e.to_glib_full() as *mut _;
            }

            ptr::null_mut()
        }
    }
}

/// Returns an input stream.  The uri can be a data: URL or a plain URI
fn acquire_stream(
    uri: &str,
    cancellable: Option<Cancellable>,
) -> Result<InputStream, LoadingError> {
    if uri.starts_with("data:") {
        let BinaryData { data, .. } = decode_data_uri(uri)?;

        let stream = MemoryInputStream::new_from_bytes(&GBytes::from_owned(data));
        Ok(stream.upcast::<InputStream>())
    } else {
        let file = GFile::new_for_uri(uri);
        let stream = file.read(cancellable.as_ref())?;

        Ok(stream.upcast::<InputStream>())
    }
}

#[no_mangle]
pub unsafe fn rsvg_io_acquire_stream(
    uri: *const libc::c_char,
    cancellable: *mut gio_sys::GCancellable,
    error: *mut *mut glib_sys::GError,
) -> *mut gio_sys::GInputStream {
    assert!(!uri.is_null());

    let uri: String = from_glib_none(uri);
    let cancellable = from_glib_borrow(cancellable);

    match acquire_stream(&uri, cancellable) {
        Ok(stream) => stream.to_glib_full(),

        Err(_e) => {
            set_gerror(error, 0, "Could not acquire stream");

            ptr::null_mut()
        }
    }
}

pub fn acquire_data(
    aurl: &AllowedUrl,
    cancellable: Option<&Cancellable>,
) -> Result<BinaryData, LoadingError> {
    let uri = aurl.url().as_str();

    if uri.starts_with("data:") {
        Ok(decode_data_uri(uri)?)
    } else {
        let file = GFile::new_for_uri(uri);
        let (contents, _etag) = file.load_contents(cancellable)?;

        let (content_type, _uncertain) = gio::content_type_guess(uri, &contents);
        let mime_type = gio::content_type_get_mime_type(&content_type);

        Ok(BinaryData {
            data: contents,
            content_type: mime_type,
        })
    }
}
