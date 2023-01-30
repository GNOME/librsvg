#![allow(unknown_lints)]
#![deny(clippy)]
#![warn(unused)]

use cairo;
use librsvg;

use anyhow::Result;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::Duration;
use structopt::{self, StructOpt};
use thiserror::Error;

#[cfg_attr(rustfmt, rustfmt_skip)]
#[derive(StructOpt, Debug)]
#[structopt(name = "rsvg-bench", about = "Benchmarking utility for librsvg.")]
struct Opt {
    #[structopt(short = "s",
                long = "sleep",
                help = "Number of seconds to sleep before starting to process SVGs",
                default_value = "0")]
    sleep_secs: usize,

    #[structopt(short = "p",
                long = "num-parse",
                help = "Number of times to parse each file",
                default_value = "100")]
    num_parse: usize,

    #[structopt(short = "r",
                long = "num-render",
                help = "Number of times to render each file",
                default_value = "100")]
    num_render: usize,

    #[structopt(help = "Input files or directories", parse(from_os_str))]
    inputs: Vec<PathBuf>,

    #[structopt(long = "hard-failures",
                help = "Whether to stop all processing when a file cannot be rendered")]
    hard_failures: bool,
}

#[derive(Debug, Error)]
enum LoadingError {
    Skipped,
    Rsvg(librsvg::LoadingError),
}

#[derive(Debug, Error)]
enum ProcessingError {
    #[error("Cairo error: {error:?}")]
    CairoError { error: cairo::Error },

    #[error("Rendering error")]
    RenderingError,
}

impl From<cairo::Error> for ProcessingError {
    fn from(error: cairo::Error) -> ProcessingError {
        ProcessingError::CairoError { error }
    }
}

impl From<librsvg::RenderingError> for ProcessingError {
    fn from(_: librsvg::RenderingError) -> ProcessingError {
        ProcessingError::RenderingError
    }
}

fn process_path<P: AsRef<Path>>(opt: &Opt, path: P) -> Result<()> {
    let meta = fs::metadata(&path)?;

    if meta.is_dir() {
        process_directory(opt, path)?;
    } else if let Some(ext) = path.as_ref().extension() {
        if ext == "svg" || ext == "SVG" {
            process_file(opt, &path)?;
        }
    }

    Ok(())
}

fn process_directory<P: AsRef<Path>>(opt: &Opt, path: P) -> Result<()> {
    println!("Processing {:?}", path.as_ref());

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        process_path(opt, &entry.path())?;
    }

    Ok(())
}

fn read_svg(opt: &Opt, path: &Path) -> Result<librsvg::SvgHandle, LoadingError> {
    match (opt.hard_failures, librsvg::Loader::new().read_path(path)) {
        (_, Ok(h)) => Ok(h),
        (false, Err(e)) => {
            println!("skipping {} due to error when loading: {}", path.to_string_lossy(), e);
            Err(LoadingError::Skipped)
        },
        (true, Err(e)) => Err(LoadingError::Rsvg(e)),
    }
}

fn process_file<P: AsRef<Path>>(opt: &Opt, path: P) -> Result<()> {
    println!("Processing {:?}", path.as_ref());

    assert!(opt.num_parse > 0);

    let path = path.as_ref();

    for _ in 0..opt.num_parse - 1 {
        match read_svg(opt, path.as_ref()) {
            Ok(_) => (),
            Err(LoadingError::Skipped) => return Ok(()),
            Err(LoadingError::Rsvg(e)) => return Err(e.into()),
        }
    }

    let handle = match read_svg(opt, path.as_ref()) {
        Ok(h) => h,
        Err(LoadingError::Skipped) => return Ok(()),
        Err(LoadingError::Rsvg(e)) => return Err(e.into()),
    };

    for _ in 0..opt.num_render {
        render_to_cairo(opt, &handle)?;
    }

    Ok(())
}

fn render_to_cairo(opt: &Opt, handle: &librsvg::SvgHandle) -> Result<(), ProcessingError> {
    let renderer = librsvg::CairoRenderer::new(handle);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100)?;
    let cr = cairo::Context::new(&surface)?;

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    match (opt.hard_failures, renderer.render_document(&cr, &viewport)) {
        (_, Ok(_)) => Ok(()),
        (false, Err(e)) => {
            println!("could not render: {}", e);
            Ok(())
        },
        (true, Err(e)) => Err(e.into()),
    }
}

fn sleep(secs: usize) {
    thread::sleep(Duration::from_secs(secs as u64))
}

fn print_options(opt: &Opt) {
    println!("Will parse each file {} times", opt.num_parse);
    println!("Will render each file {} times", opt.num_render);
    if opt.num_render > 0 {
        println!("Rendering to Cairo image surface");
    }
    println!("Sleeping for {} seconds before processing SVGs...",
             opt.sleep_secs);
}

fn run(opt: &Opt) -> Result<()> {
    print_options(opt);

    sleep(opt.sleep_secs);
    println!("Processing files!");

    for path in &opt.inputs {
        process_path(opt, &path)?;
    }

    Ok(())
}

fn main() {
    let opt = Opt::from_args();

    if opt.inputs.is_empty() {
        eprintln!("No input files or directories specified\n");

        let app = Opt::clap();
        let mut out = io::stderr();
        app.write_help(&mut out).expect("failed to write to stderr");
        eprintln!("");
        process::exit(1);
    }

    if opt.num_parse < 1 {
        eprintln!("Must parse files at least 1 time; please specify a higher number\n");
        process::exit(1);
    }

    println!("hard_failures: {:?}", opt.hard_failures);

    match run(&opt) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
