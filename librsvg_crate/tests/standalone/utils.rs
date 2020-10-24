#![allow(unused)]
use cairo;
use gio;
use glib;

use librsvg::{CairoRenderer, Loader, LoadingError, RenderingError, SvgHandle};

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

pub fn load_svg(input: &'static [u8]) -> Result<SvgHandle, LoadingError> {
    let bytes = glib::Bytes::from_static(input);
    let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

    Loader::new().read_stream(&stream, None::<&gio::File>, None::<&gio::Cancellable>)
}

#[derive(Copy, Clone)]
pub struct SurfaceSize(pub i32, pub i32);

pub fn render_document<F: FnOnce(&cairo::Context)>(
    svg: &SvgHandle,
    surface_size: SurfaceSize,
    cr_transform: F,
    viewport: cairo::Rectangle,
) -> Result<SharedImageSurface, RenderingError> {
    let renderer = CairoRenderer::new(svg);

    let SurfaceSize(width, height) = surface_size;

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();

    let res = {
        let cr = cairo::Context::new(&output);
        cr_transform(&cr);
        Ok(renderer.render_document(&cr, &viewport)?)
    };

    res.and_then(|_| Ok(SharedImageSurface::wrap(output, SurfaceType::SRgb)?))
}
