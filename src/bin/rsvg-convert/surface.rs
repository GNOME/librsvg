use core::ops::Deref;
use std::io;

use librsvg::{CairoRenderer, RenderingError};

use crate::cli;
use crate::output::Stream;

pub enum Surface {
    Png(cairo::ImageSurface, Stream),
    Pdf(cairo::PdfSurface, (f64, f64)),
}

impl Deref for Surface {
    type Target = cairo::Surface;

    fn deref(&self) -> &cairo::Surface {
        match self {
            Self::Png(surface, _) => surface.deref(),
            Self::Pdf(surface, _) => surface.deref(),
        }
    }
}

impl Surface {
    pub fn new(
        format: cli::Format,
        width: f64,
        height: f64,
        stream: Stream,
    ) -> Result<Self, cairo::Status> {
        match format {
            cli::Format::Png => {
                cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32)
                    .map(|s| Self::Png(s, stream))
            }
            cli::Format::Pdf => cairo::PdfSurface::for_stream(width, height, stream)
                .map(|s| Self::Pdf(s, (width, height))),
            _ => Err(cairo::Status::InvalidFormat),
        }
    }

    fn size(&self) -> (f64, f64) {
        match self {
            Self::Png(s, _) => (s.get_width() as f64, s.get_height() as f64),
            Self::Pdf(_, size) => *size,
        }
    }

    pub fn render(&self, renderer: &CairoRenderer, id: Option<&str>) -> Result<(), RenderingError> {
        let cr = cairo::Context::new(self);

        let (width, height) = self.size();
        let viewport = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        };

        renderer.render_layer(&cr, id, &viewport)?;
        cr.show_page();

        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), cairo::IoError> {
        match self {
            Self::Png(surface, stream) => surface.write_to_png(stream),
            Self::Pdf(surface, _) => surface
                .finish_output_stream()
                .map(|_| ())
                .map_err(|e| cairo::IoError::Io(io::Error::from(e))),
        }
    }
}
