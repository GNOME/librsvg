use core::ops::Deref;
use std::io;

use librsvg::{CairoRenderer, RenderingError};

use crate::cli::Format;
use crate::output::Stream;

pub enum Surface {
    Png(cairo::ImageSurface, Stream),
    Pdf(cairo::PdfSurface, (f64, f64)),
    Ps(cairo::PsSurface, (f64, f64)),
    Svg(cairo::SvgSurface, (f64, f64)),
}

impl Deref for Surface {
    type Target = cairo::Surface;

    fn deref(&self) -> &cairo::Surface {
        match self {
            Self::Png(surface, _) => &surface,
            Self::Pdf(surface, _) => &surface,
            Self::Ps(surface, _) => &surface,
            Self::Svg(surface, _) => &surface,
        }
    }
}

impl Surface {
    pub fn new(
        format: Format,
        width: f64,
        height: f64,
        stream: Stream,
    ) -> Result<Self, cairo::Status> {
        match format {
            Format::Png => Self::new_for_png(width, height, stream),
            Format::Pdf => Self::new_for_pdf(width, height, stream),
            Format::Ps => Self::new_for_ps(width, height, stream, false),
            Format::Eps => Self::new_for_ps(width, height, stream, true),
            Format::Svg => Self::new_for_svg(width, height, stream),
        }
    }

    fn new_for_png(width: f64, height: f64, stream: Stream) -> Result<Self, cairo::Status> {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32)?;
        Ok(Self::Png(surface, stream))
    }

    fn new_for_pdf(width: f64, height: f64, stream: Stream) -> Result<Self, cairo::Status> {
        let surface = cairo::PdfSurface::for_stream(width, height, stream)?;
        Ok(Self::Pdf(surface, (width, height)))
    }

    fn new_for_ps(
        width: f64,
        height: f64,
        stream: Stream,
        eps: bool,
    ) -> Result<Self, cairo::Status> {
        let surface = cairo::PsSurface::for_stream(width, height, stream)?;
        surface.set_eps(eps);
        Ok(Self::Ps(surface, (width, height)))
    }

    fn new_for_svg(width: f64, height: f64, stream: Stream) -> Result<Self, cairo::Status> {
        let surface = cairo::SvgSurface::for_stream(width, height, stream)?;
        Ok(Self::Svg(surface, (width, height)))
    }

    fn size(&self) -> (f64, f64) {
        match self {
            Self::Png(s, _) => (s.get_width() as f64, s.get_height() as f64),
            Self::Pdf(_, size) => *size,
            Self::Ps(_, size) => *size,
            Self::Svg(_, size) => *size,
        }
    }

    fn bounds(&self) -> cairo::Rectangle {
        let (width, height) = self.size();
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    pub fn render(
        &self,
        renderer: &CairoRenderer,
        cr: &cairo::Context,
        id: Option<&str>,
    ) -> Result<(), RenderingError> {
        let show_page = |_| self.show_page(cr);
        renderer
            .render_layer(cr, id, &self.bounds())
            .and_then(show_page)
    }

    pub fn show_page(&self, cr: &cairo::Context) -> Result<(), RenderingError> {
        match self {
            Self::Png(_, _) => (),
            _ => cr.show_page(),
        }
        Ok(())
    }

    fn finish_output_stream(surface: &cairo::Surface) -> Result<(), cairo::IoError> {
        match surface.finish_output_stream() {
            Ok(_) => Ok(()),
            Err(e) => Err(cairo::IoError::Io(io::Error::from(e))),
        }
    }

    pub fn finish(&mut self) -> Result<(), cairo::IoError> {
        match self {
            Self::Png(surface, stream) => surface.write_to_png(stream),
            _ => Self::finish_output_stream(self),
        }
    }
}
