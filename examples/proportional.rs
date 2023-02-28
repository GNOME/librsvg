use cairo;
use rsvg;

use std::fs::File;
use std::io::BufWriter;
use std::process;

fn main() {
    let args = std::env::args_os();

    if args.len() != 5 {
        eprintln!("usage: render <input.svg> <width> <height> <output.png>");
        process::exit(1);
    }

    let mut args = args.skip(1);

    let input = args.next().unwrap();
    let width_os = args.next().unwrap();
    let height_os = args.next().unwrap();
    let output = args.next().unwrap();

    let width_s = width_os.to_str().unwrap();
    let height_s = height_os.to_str().unwrap();

    let width: i32 = width_s.parse().unwrap();
    let height: i32 = height_s.parse().unwrap();

    assert!(width > 0 && height > 0);

    let handle = match rsvg::Loader::new().read_path(input) {
        Ok(handle) => handle,

        Err(e) => {
            eprintln!("loading error: {}", e);
            process::exit(1);
        }
    };

    let renderer = rsvg::CairoRenderer::new(&handle);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
    let cr = cairo::Context::new(&surface).expect("Failed to create a cairo context");

    let res = renderer.render_document(
        &cr,
        &cairo::Rectangle::new(0.0, 0.0, f64::from(width), f64::from(height)),
    );

    match res {
        Ok(()) => {
            let mut file = BufWriter::new(File::create(output).unwrap());

            surface.write_to_png(&mut file).unwrap();
        }

        Err(e) => {
            eprintln!("rendering error: {}", e);
            process::exit(1);
        }
    }
}
