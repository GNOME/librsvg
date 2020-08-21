//! Utilities to acquire streams and data from from URLs.

use gio::{
    BufferedInputStream, BufferedInputStreamExt, Cancellable, ConverterInputStream, File as GFile,
    FileExt, InputStream, MemoryInputStream, ZlibCompressorFormat, ZlibDecompressor,
};
use glib::{Bytes as GBytes, Cast};

use crate::allowed_url::AllowedUrl;
use crate::error::LoadingError;

pub struct BinaryData {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
}

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

// Header of a gzip data stream
const GZ_MAGIC_0: u8 = 0x1f;
const GZ_MAGIC_1: u8 = 0x8b;

pub fn get_input_stream_for_loading(
    stream: &InputStream,
    cancellable: Option<&Cancellable>,
) -> Result<InputStream, LoadingError> {
    // detect gzipped streams (svgz)

    let buffered = BufferedInputStream::new(stream);
    let num_read = buffered.fill(2, cancellable)?;
    if num_read < 2 {
        // FIXME: this string was localized in the original; localize it
        return Err(LoadingError::XmlParseError(String::from(
            "Input file is too short",
        )));
    }

    let buf = buffered.peek_buffer();
    assert!(buf.len() >= 2);
    if buf[0..2] == [GZ_MAGIC_0, GZ_MAGIC_1] {
        let decomp = ZlibDecompressor::new(ZlibCompressorFormat::Gzip);
        let converter = ConverterInputStream::new(&buffered, &decomp);
        Ok(converter.upcast::<InputStream>())
    } else {
        Ok(buffered.upcast::<InputStream>())
    }
}

/// Returns an input stream.  The url can be a data: URL or a plain URI
pub fn acquire_stream(
    aurl: &AllowedUrl,
    cancellable: Option<&Cancellable>,
) -> Result<InputStream, LoadingError> {
    let uri = aurl.as_str();

    if uri.starts_with("data:") {
        let BinaryData { data, .. } = decode_data_uri(uri)?;

        //        {
        //            use std::fs::File;
        //            use std::io::prelude::*;
        //
        //            let mut file = File::create("data.bin").unwrap();
        //            file.write_all(&data).unwrap();
        //        }

        let stream = MemoryInputStream::new_from_bytes(&GBytes::from_owned(data));
        Ok(stream.upcast::<InputStream>())
    } else {
        let file = GFile::new_for_uri(uri);
        let stream = file.read(cancellable)?;

        Ok(stream.upcast::<InputStream>())
    }
}

/// Returns a chunk of data.  The url can be a data: URL or a plain URI
pub fn acquire_data(
    aurl: &AllowedUrl,
    cancellable: Option<&Cancellable>,
) -> Result<BinaryData, LoadingError> {
    let uri = aurl.as_str();

    if uri.starts_with("data:") {
        Ok(decode_data_uri(uri)?)
    } else {
        let file = GFile::new_for_uri(uri);
        let (contents, _etag) = file.load_contents(cancellable)?;

        let (content_type, _uncertain) = gio::content_type_guess(Some(uri), &contents);
        let mime_type = gio::content_type_get_mime_type(&content_type).map(String::from);

        Ok(BinaryData {
            data: contents,
            content_type: mime_type.map(From::from),
        })
    }
}
