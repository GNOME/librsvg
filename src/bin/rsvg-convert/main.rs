#[macro_use]
extern crate clap;

mod cli;
mod input;
mod output;
mod surface;

use librsvg::{CairoRenderer, Loader};

use crate::cli::Args;
use crate::output::Stream;
use crate::surface::Surface;

fn load_stylesheet(args: &Args) -> std::io::Result<Option<String>> {
    match args.stylesheet {
        Some(ref filename) => std::fs::read_to_string(filename).map(Some),
        None => Ok(None),
    }
}

fn main() {
    let args = Args::new().unwrap_or_else(|e| e.exit());

    let stylesheet = load_stylesheet(&args).expect("could not load stylesheet");
    let mut target = None;

    for input in args.input() {
        let mut handle = Loader::new()
            .with_unlimited_size(args.unlimited)
            .keep_image_data(args.keep_image_data)
            .read_stream(input.stream(), input.file(), None::<&gio::Cancellable>)
            .expect("error loading SVG file");

        if let Some(ref css) = stylesheet {
            handle.set_stylesheet(&css).unwrap();
        }

        let renderer = CairoRenderer::new(&handle).with_dpi(args.dpi_x, args.dpi_y);

        if target.is_none() {
            target = match renderer.intrinsic_size_in_pixels() {
                Some((width, height)) => {
                    let output = Stream::new(args.output()).unwrap();
                    Some(Surface::new(args.format, width, height, output).unwrap())
                }
                None => None,
            };
        }

        if let Some(ref surface) = target {
            surface
                .render(&renderer, args.export_id.as_deref())
                .unwrap();
        }
    }

    if let Some(ref mut surface) = target {
        surface.finish().unwrap();
    }
}
