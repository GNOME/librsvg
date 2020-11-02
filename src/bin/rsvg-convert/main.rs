#[macro_use]
extern crate clap;

mod cli;
mod input;
mod output;
mod surface;

use cssparser::Color;
use librsvg::{CairoRenderer, Loader, RenderingError};

use crate::cli::Args;
use crate::output::Stream;
use crate::surface::Surface;

macro_rules! exit {
    () => (exit!("Error"));
    ($($arg:tt)*) => ({
        std::eprintln!("{}", std::format_args!($($arg)*));
        std::process::exit(1);
    })
}

fn load_stylesheet(args: &Args) -> std::io::Result<Option<String>> {
    match args.stylesheet {
        Some(ref filename) => std::fs::read_to_string(filename).map(Some),
        None => Ok(None),
    }
}

fn main() {
    let args = Args::new().unwrap_or_else(|e| e.exit());

    let stylesheet =
        load_stylesheet(&args).unwrap_or_else(|e| exit!("Error reading stylesheet: {}", e));

    let mut target = None;

    for input in args.input() {
        let mut handle = Loader::new()
            .with_unlimited_size(args.unlimited)
            .keep_image_data(args.keep_image_data)
            .read_stream(input.stream(), input.file(), None::<&gio::Cancellable>)
            .unwrap_or_else(|e| exit!("Error reading SVG {}: {}", input, e));

        if let Some(ref css) = stylesheet {
            handle
                .set_stylesheet(&css)
                .unwrap_or_else(|e| exit!("Error applying stylesheet: {}", e));
        }

        let renderer = CairoRenderer::new(&handle).with_dpi(args.dpi_x, args.dpi_y);

        if target.is_none() {
            target = match renderer.intrinsic_size_in_pixels() {
                Some((width, height)) => {
                    let output = Stream::new(args.output())
                        .unwrap_or_else(|e| exit!("Error opening output: {}", e));

                    Some(
                        Surface::new(args.format, width, height, output)
                            .unwrap_or_else(|e| exit!("Error creating output surface: {}", e)),
                    )
                }
                None => None,
            };
        }

        if let Some(ref surface) = target {
            let cr = cairo::Context::new(surface);

            if let Some(Color::RGBA(rgba)) = args.background_color {
                cr.set_source_rgba(
                    rgba.red_f32().into(),
                    rgba.green_f32().into(),
                    rgba.blue_f32().into(),
                    rgba.alpha_f32().into(),
                );
            }

            surface
                .render(&renderer, &cr, args.export_id())
                .unwrap_or_else(|e| match e {
                    RenderingError::InvalidId(_) => exit!(
                        "File {} does not have an object with id \"{}\")",
                        input,
                        args.export_id().unwrap()
                    ),
                    _ => exit!("Error rendering SVG {}: {}", input, e),
                });
        }
    }

    if let Some(ref mut surface) = target {
        surface
            .finish()
            .unwrap_or_else(|e| exit!("Error saving output: {}", e));
    }
}
