use core::ops::Deref;
use std::io;

use librsvg::{CairoRenderer, RenderingError};

use crate::cli::Format;
use crate::output::Stream;
use crate::size::Size;

// TODO: use from src/lib/handle.rs
fn checked_i32(x: f64) -> Result<i32, cairo::Status> {
    cast::i32(x).map_err(|_| cairo::Status::InvalidSize)
}

pub enum Surface {
    Png(cairo::ImageSurface, Stream),
    Pdf(cairo::PdfSurface, Size),
    Ps(cairo::PsSurface, Size),
    Svg(cairo::SvgSurface, Size),
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
    pub fn new(format: Format, size: Size, stream: Stream) -> Result<Self, cairo::Status> {
        match format {
            Format::Png => Self::new_for_png(size, stream),
            Format::Pdf => Self::new_for_pdf(size, stream),
            Format::Ps => Self::new_for_ps(size, stream, false),
            Format::Eps => Self::new_for_ps(size, stream, true),
            Format::Svg => Self::new_for_svg(size, stream),
        }
    }

    fn new_for_png(size: Size, stream: Stream) -> Result<Self, cairo::Status> {
        let w = checked_i32(size.w.round())?;
        let h = checked_i32(size.h.round())?;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
        Ok(Self::Png(surface, stream))
    }

    fn new_for_pdf(size: Size, stream: Stream) -> Result<Self, cairo::Status> {
        let surface = cairo::PdfSurface::for_stream(size.w, size.h, stream)?;
        if let Some(date) = metadata::creation_date() {
            surface.set_metadata(cairo::PdfMetadata::CreateDate, &date)?;
        }
        Ok(Self::Pdf(surface, size))
    }

    fn new_for_ps(size: Size, stream: Stream, eps: bool) -> Result<Self, cairo::Status> {
        let surface = cairo::PsSurface::for_stream(size.w, size.h, stream)?;
        surface.set_eps(eps);
        Ok(Self::Ps(surface, size))
    }

    fn new_for_svg(size: Size, stream: Stream) -> Result<Self, cairo::Status> {
        let surface = cairo::SvgSurface::for_stream(size.w, size.h, stream)?;
        Ok(Self::Svg(surface, size))
    }

    fn size(&self) -> Size {
        match self {
            Self::Png(surface, _) => Size {
                w: surface.get_width() as f64,
                h: surface.get_height() as f64,
            },
            Self::Pdf(_, size) => size.clone(),
            Self::Ps(_, size) => size.clone(),
            Self::Svg(_, size) => size.clone(),
        }
    }

    fn bounds(&self) -> cairo::Rectangle {
        let size = self.size();
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: size.w,
            height: size.h,
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

mod metadata {
    use chrono::prelude::*;
    use std::env;
    use std::str::FromStr;

    use super::super::exit;

    pub fn creation_date() -> Option<String> {
        match env::var("SOURCE_DATE_EPOCH") {
            Ok(epoch) => {
                let seconds = i64::from_str(&epoch)
                    .unwrap_or_else(|e| exit!("Environment variable $SOURCE_DATE_EPOCH: {}", e));
                let datetime = Utc.timestamp(seconds, 0);
                Some(datetime.to_rfc3339())
            }
            Err(env::VarError::NotPresent) => None,
            Err(env::VarError::NotUnicode(_)) => {
                exit!("Environment variable $SOURCE_DATE_EPOCH is not valid Unicode")
            }
        }
    }
}
