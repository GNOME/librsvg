use gio;
use glib::translate::*;
use glib::{Bytes, Cast};
use glib_sys;

use std::slice;

use error::set_gerror;
use handle::LoadOptions;
use xml::XmlState;
use xml2_load::{xml_state_load_from_possibly_compressed_stream, ParseFromStreamError};

// Long-lived loading context for the deprecated I/O API
//
// rsvg_handle_write() and rsvg_handle_close() are old-style functions to
// feed an RsvgHandle with data.  Current code prefers the stream APIs,
// rsvg_handle_new_from_stream_sync() and similar.
//
// This struct maintains the loading context while an RsvgHandle is being
// populated with data, in case the caller is using write()/close().
pub struct LoadContext<'a> {
    load_options: LoadOptions,

    state: LoadState,

    buffer: Vec<u8>,

    xml: &'a mut XmlState,
}

#[derive(Copy, Clone)]
enum LoadState {
    Start,
    Reading,
    Closed,
}

impl<'a> LoadContext<'a> {
    pub fn new(xml: &mut XmlState, load_options: LoadOptions) -> LoadContext {
        LoadContext {
            load_options,
            state: LoadState::Start,
            buffer: Vec::new(),
            xml,
        }
    }

    pub fn write(&mut self, buf: &[u8]) {
        let state = self.state;

        self.state = match state {
            LoadState::Start => LoadState::Reading,
            LoadState::Reading => LoadState::Reading,

            _ => unreachable!(),
        };

        self.buffer.extend_from_slice(buf);
    }

    pub fn close(&mut self) -> Result<(), ParseFromStreamError> {
        let state = self.state;

        match state {
            LoadState::Start | LoadState::Closed => {
                self.state = LoadState::Closed;
                Ok(())
            }

            LoadState::Reading => {
                self.state = LoadState::Closed;

                let bytes = Bytes::from(&self.buffer);
                let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

                xml_state_load_from_possibly_compressed_stream(
                    &mut self.xml,
                    &self.load_options,
                    stream.upcast(),
                    None,
                )
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_load_new<'a>(
    raw_xml: *mut XmlState,
    flags: u32,
) -> *mut LoadContext<'a> {
    assert!(!raw_xml.is_null());

    let xml = &mut *raw_xml;
    let load_options = LoadOptions::from_flags(flags);

    Box::into_raw(Box::new(LoadContext::new(xml, load_options)))
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_load_free(raw_load_ctx: *mut LoadContext) -> *mut XmlState {
    assert!(!raw_load_ctx.is_null());

    let load_ctx = &mut *raw_load_ctx;

    let xml = load_ctx.xml as *mut _;

    Box::from_raw(raw_load_ctx);

    xml
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_load_write(
    raw_load_ctx: *mut LoadContext,
    buf: *const u8,
    size: usize,
) {
    assert!(!raw_load_ctx.is_null());

    let load_ctx = &mut *raw_load_ctx;
    let slice = slice::from_raw_parts(buf, size);

    load_ctx.write(slice);
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_load_close(
    raw_load_ctx: *mut LoadContext,
    error: *mut *mut glib_sys::GError,
) -> glib_sys::gboolean {
    assert!(!raw_load_ctx.is_null());

    let load_ctx = &mut *raw_load_ctx;

    match load_ctx.close() {
        Ok(()) => true.to_glib(),

        Err(e) => {
            match e {
                ParseFromStreamError::CouldNotCreateParser => {
                    set_gerror(error, 0, "Error creating XML parser");
                }

                ParseFromStreamError::IoError(e) => {
                    if !error.is_null() {
                        *error = e.to_glib_full() as *mut _;
                    }
                }

                ParseFromStreamError::XmlParseError(s) => {
                    set_gerror(error, 0, &s);
                }
            }

            false.to_glib()
        }
    }
}
