use gio;
use glib::{Bytes, Cast};

use handle::{LoadOptions, RsvgHandle};
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
pub struct LoadContext {
    load_options: LoadOptions,

    state: LoadState,

    buffer: Vec<u8>,

    xml: Option<XmlState>,
}

#[derive(Copy, Clone)]
enum LoadState {
    Start,
    Reading,
    Closed,
}

impl LoadContext {
    pub fn new(handle: *mut RsvgHandle, load_options: LoadOptions) -> LoadContext {
        LoadContext {
            load_options,
            state: LoadState::Start,
            buffer: Vec::new(),
            xml: Some(XmlState::new(handle)),
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

    pub fn close(&mut self) -> Result<XmlState, ParseFromStreamError> {
        let state = self.state;

        match state {
            LoadState::Start => {
                self.state = LoadState::Closed;
                Ok(self.xml.take().unwrap())
            }

            LoadState::Reading => {
                self.state = LoadState::Closed;

                let bytes = Bytes::from(&self.buffer);
                let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

                xml_state_load_from_possibly_compressed_stream(
                    self.xml.as_mut().unwrap(),
                    &self.load_options,
                    &stream.upcast(),
                    None,
                )
                .and_then(|_| Ok(self.xml.take().unwrap()))
            }

            LoadState::Closed => unreachable!(),
        }
    }
}
