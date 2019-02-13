extern crate cairo;
extern crate librsvg;

use std::fs::File;
use std::io::BufWriter;
use std::process;

fn main() {
    let args = std::env::args_os();

    if args.len() != 3 {
        eprintln!("usage: render <input.svg> <output.png>");
        process::exit(1);
    }

    let mut args = args.skip(1);

    let input = args.next().unwrap();
    let output = args.next().unwrap();

    let handle = librsvg::LoadOptions::new().read_path(input).unwrap();

    let renderer = handle.get_cairo_renderer();

    let (w, h) = renderer.get_dimensions().unwrap();

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).unwrap();
    let cr = cairo::Context::new(&surface);
    renderer.render(&cr).unwrap();

    let mut file = BufWriter::new(File::create(output).unwrap());

    surface.write_to_png(&mut file).unwrap();
}
