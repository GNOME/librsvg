#![allow(unknown_lints)]
#![deny(clippy)]
#![warn(unused)]

use clap::{crate_version, value_parser};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
/// Command-line options for `rsvg-bench`.
struct Opt {
    /// Number of seconds to sleep before starting to process SVGs.
    sleep_secs: usize,

    /// Number of times to parse each file.
    num_parse: usize,

    /// Number of times to render each file.
    num_render: usize,

    /// Whether to stop all processing when a file cannot be rendered.
    hard_failures: bool,

    /// Input files or directories.
    inputs: Vec<PathBuf>,
}

#[derive(Debug)]
enum LoadingError {
    Skipped,
    Rsvg(rsvg::LoadingError),
}

#[derive(Debug)]
enum ProcessingError {
    Rsvg(rsvg::LoadingError),

    CairoError { error: cairo::Error },

    RenderingError,
}

impl From<io::Error> for ProcessingError {
    fn from(error: io::Error) -> ProcessingError {
        ProcessingError::Rsvg(rsvg::LoadingError::Io(format!("{error}")))
    }
}

impl From<cairo::Error> for ProcessingError {
    fn from(error: cairo::Error) -> ProcessingError {
        ProcessingError::CairoError { error }
    }
}

impl From<rsvg::LoadingError> for ProcessingError {
    fn from(error: rsvg::LoadingError) -> ProcessingError {
        ProcessingError::Rsvg(error)
    }
}

impl From<LoadingError> for ProcessingError {
    fn from(error: LoadingError) -> ProcessingError {
        match error {
            LoadingError::Skipped => {
                unreachable!("calling code should have caught a LoadingError::Skipped")
            }
            LoadingError::Rsvg(e) => ProcessingError::Rsvg(e),
        }
    }
}

impl From<rsvg::RenderingError> for ProcessingError {
    fn from(_: rsvg::RenderingError) -> ProcessingError {
        ProcessingError::RenderingError
    }
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessingError::Rsvg(e) => write!(f, "{e}"),
            ProcessingError::CairoError { error } => write!(f, "{error}"),
            ProcessingError::RenderingError => write!(f, "rendering error"),
        }
    }
}

fn process_path<P: AsRef<Path>>(opt: &Opt, path: P) -> Result<(), ProcessingError> {
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

fn process_directory<P: AsRef<Path>>(opt: &Opt, path: P) -> Result<(), ProcessingError> {
    println!("Processing {:?}", path.as_ref());

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        process_path(opt, &entry.path())?;
    }

    Ok(())
}

fn read_svg(opt: &Opt, path: &Path) -> Result<rsvg::SvgHandle, LoadingError> {
    match (opt.hard_failures, rsvg::Loader::new().read_path(path)) {
        (_, Ok(h)) => Ok(h),
        (false, Err(e)) => {
            println!(
                "skipping {} due to error when loading: {}",
                path.to_string_lossy(),
                e
            );
            Err(LoadingError::Skipped)
        }
        (true, Err(e)) => Err(LoadingError::Rsvg(e)),
    }
}

fn process_file<P: AsRef<Path>>(opt: &Opt, path: P) -> Result<(), ProcessingError> {
    println!("Processing {:?}", path.as_ref());

    assert!(opt.num_parse > 0);

    let path = path.as_ref();

    for _ in 0..opt.num_parse - 1 {
        match read_svg(opt, path) {
            Ok(_) => (),
            Err(LoadingError::Skipped) => return Ok(()),
            Err(LoadingError::Rsvg(e)) => return Err(e.into()),
        }
    }

    let handle = match read_svg(opt, path) {
        Ok(h) => h,
        Err(LoadingError::Skipped) => return Ok(()),
        Err(LoadingError::Rsvg(e)) => return Err(e.into()),
    };

    for _ in 0..opt.num_render {
        render_to_cairo(opt, &handle)?;
    }

    Ok(())
}

fn render_to_cairo(opt: &Opt, handle: &rsvg::SvgHandle) -> Result<(), ProcessingError> {
    let renderer = rsvg::CairoRenderer::new(handle);

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100)?;
    let cr = cairo::Context::new(&surface)?;

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    match (opt.hard_failures, renderer.render_document(&cr, &viewport)) {
        (_, Ok(_)) => Ok(()),
        (false, Err(e)) => {
            println!("could not render: {e}");
            Ok(())
        }
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
    println!(
        "Sleeping for {} seconds before processing SVGs...",
        opt.sleep_secs
    );
}

fn run(opt: &Opt) -> Result<(), ProcessingError> {
    print_options(opt);

    sleep(opt.sleep_secs);
    println!("Processing files!");

    for path in &opt.inputs {
        process_path(opt, path)?;
    }

    Ok(())
}

fn build_cli() -> clap::Command {
    clap::Command::new("rsvg-bench")
        .version(concat!("version ", crate_version!()))
        .about("Benchmarking utility for librsvg.")
        .arg(
            clap::Arg::new("sleep")
                .long("sleep")
                .help("Number of seconds to sleep before starting to process SVGs")
                .default_value("0")
                .value_parser(str::parse::<usize>),
        )
        .arg(
            clap::Arg::new("num-parse")
                .long("num-parse")
                .help("Number of times to parse each file")
                .default_value("1")
                .value_parser(str::parse::<usize>),
        )
        .arg(
            clap::Arg::new("num-render")
                .long("num-render")
                .help("Number of times to render each file")
                .default_value("1")
                .value_parser(str::parse::<usize>),
        )
        .arg(
            clap::Arg::new("hard-failures")
                .long("hard-failures")
                .help("Stop all processing when a file cannot be rendered")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("inputs")
                .help("Input files or directories")
                .value_parser(value_parser!(PathBuf))
                .action(clap::ArgAction::Append),
        )
}

fn main() {
    let cli = build_cli();

    let matches = cli.get_matches();

    let sleep_secs = matches
        .get_one("sleep")
        .copied()
        .expect("already provided default_value");
    let num_parse = matches
        .get_one("num-parse")
        .copied()
        .expect("already provided default_value");
    let num_render = matches
        .get_one("num-render")
        .copied()
        .expect("already provided default_value");
    let hard_failures = matches.get_flag("hard-failures");

    let inputs = if let Some(inputs) = matches.get_many("inputs") {
        inputs.cloned().collect()
    } else {
        eprintln!("Must specify at least one SVG file or directory to process\n");
        process::exit(1);
    };

    let opt = Opt {
        sleep_secs,
        num_parse,
        num_render,
        hard_failures,
        inputs,
    };

    if opt.num_parse < 1 {
        eprintln!("Must parse files at least 1 time; please specify a higher number\n");
        process::exit(1);
    }

    println!("hard_failures: {:?}", opt.hard_failures);

    match run(&opt) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
