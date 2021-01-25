#[macro_use]
extern crate clap;

use gio::prelude::*;
use gio::{
    Cancellable, FileCreateFlags, FileExt, InputStream, OutputStream, UnixInputStream,
    UnixOutputStream,
};
use librsvg::rsvg_convert_only::{LegacySize, PathOrUrl};
use librsvg::{CairoRenderer, Color, Loader, Parse, RenderingError};
use once_cell::unsync::OnceCell;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
struct Scale {
    pub x: f64,
    pub y: f64,
}

impl Scale {
    #[allow(clippy::float_cmp)]
    pub fn is_identity(&self) -> bool {
        self.x == 1.0 && self.y == 1.0
    }
}

#[derive(Clone, Copy, Debug)]
struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }
}

#[derive(Clone, Copy, Debug)]
enum ResizeStrategy {
    Scale(Scale),
    Fit(u32, u32),
    FitWidth(u32),
    FitHeight(u32),
    FitLargestScale(Scale, Option<u32>, Option<u32>),
}

impl ResizeStrategy {
    pub fn apply(self, input: Size, keep_aspect_ratio: bool) -> Option<Size> {
        if input.w == 0.0 || input.h == 0.0 {
            return None;
        }

        let output = match self {
            ResizeStrategy::Scale(s) => Size {
                w: input.w * s.x,
                h: input.h * s.y,
            },
            ResizeStrategy::Fit(w, h) => Size {
                w: f64::from(w),
                h: f64::from(h),
            },
            ResizeStrategy::FitWidth(w) => Size {
                w: f64::from(w),
                h: input.h * f64::from(w) / input.w,
            },
            ResizeStrategy::FitHeight(h) => Size {
                w: input.w * f64::from(h) / input.h,
                h: f64::from(h),
            },
            ResizeStrategy::FitLargestScale(s, w, h) => {
                let scaled_input_w = input.w * s.x;
                let scaled_input_h = input.h * s.y;

                let f = match (w.map(f64::from), h.map(f64::from)) {
                    (Some(w), Some(h)) if w < scaled_input_w || h < scaled_input_h => {
                        let sx = w / scaled_input_w;
                        let sy = h / scaled_input_h;
                        if sx > sy {
                            sy
                        } else {
                            sx
                        }
                    }
                    (Some(w), None) if w < scaled_input_w => w / scaled_input_w,
                    (None, Some(h)) if h < scaled_input_h => h / scaled_input_h,
                    _ => 1.0,
                };

                Size {
                    w: input.w * f * s.x,
                    h: input.h * f * s.y,
                }
            }
        };

        if !keep_aspect_ratio {
            Some(output)
        } else if output.w < output.h {
            Some(Size {
                w: output.w,
                h: input.h * (output.w / input.w),
            })
        } else {
            Some(Size {
                w: input.w * (output.h / input.h),
                h: output.h,
            })
        }
    }
}

enum Surface {
    Png(cairo::ImageSurface, OutputStream),
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
    pub fn new(format: Format, size: Size, stream: OutputStream) -> Result<Self, cairo::Status> {
        match format {
            Format::Png => Self::new_for_png(size, stream),
            Format::Pdf => Self::new_for_pdf(size, stream),
            Format::Ps => Self::new_for_ps(size, stream, false),
            Format::Eps => Self::new_for_ps(size, stream, true),
            Format::Svg => Self::new_for_svg(size, stream),
        }
    }

    fn new_for_png(size: Size, stream: OutputStream) -> Result<Self, cairo::Status> {
        let w = checked_i32(size.w.round())?;
        let h = checked_i32(size.h.round())?;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
        Ok(Self::Png(surface, stream))
    }

    fn new_for_pdf(size: Size, stream: OutputStream) -> Result<Self, cairo::Status> {
        let surface = cairo::PdfSurface::for_stream(size.w, size.h, stream.into_write())?;
        if let Some(date) = metadata::creation_date() {
            surface.set_metadata(cairo::PdfMetadata::CreateDate, &date)?;
        }
        Ok(Self::Pdf(surface, size))
    }

    fn new_for_ps(size: Size, stream: OutputStream, eps: bool) -> Result<Self, cairo::Status> {
        let surface = cairo::PsSurface::for_stream(size.w, size.h, stream.into_write())?;
        surface.set_eps(eps);
        Ok(Self::Ps(surface, size))
    }

    fn new_for_svg(size: Size, stream: OutputStream) -> Result<Self, cairo::Status> {
        let surface = cairo::SvgSurface::for_stream(size.w, size.h, stream.into_write())?;
        Ok(Self::Svg(surface, size))
    }

    fn bounds(&self) -> cairo::Rectangle {
        let (w, h) = match self {
            Self::Png(surface, _) => (
                f64::from(surface.get_width()),
                f64::from(surface.get_height()),
            ),
            Self::Pdf(_, size) => (size.w, size.h),
            Self::Ps(_, size) => (size.w, size.h),
            Self::Svg(_, size) => (size.w, size.h),
        };

        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
        }
    }

    pub fn render(
        &self,
        renderer: &CairoRenderer,
        scale: Scale,
        background_color: Option<Color>,
        id: Option<&str>,
    ) -> Result<(), RenderingError> {
        let cr = cairo::Context::new(self);

        if let Some(Color::RGBA(rgba)) = background_color {
            cr.set_source_rgba(
                rgba.red_f32().into(),
                rgba.green_f32().into(),
                rgba.blue_f32().into(),
                rgba.alpha_f32().into(),
            );

            cr.paint();
        }

        cr.scale(scale.x, scale.y);

        renderer.render_layer(&cr, id, &self.bounds()).map(|_| {
            if !matches!(self, Self::Png(_, _)) {
                cr.show_page();
            }
        })
    }

    pub fn finish(self) -> Result<(), cairo::IoError> {
        match self {
            Self::Png(surface, stream) => surface.write_to_png(&mut stream.into_write()),
            _ => match self.finish_output_stream() {
                Ok(_) => Ok(()),
                Err(e) => Err(cairo::IoError::Io(std::io::Error::from(e))),
            },
        }
    }
}

fn checked_i32(x: f64) -> Result<i32, cairo::Status> {
    cast::i32(x).map_err(|_| cairo::Status::InvalidSize)
}

mod metadata {
    use chrono::prelude::*;
    use std::env;
    use std::str::FromStr;

    use super::exit;

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

// These Stdin and Stdout types can be removed once we depend on Rust 1.48

struct Stdin;

impl Stdin {
    pub fn stream() -> UnixInputStream {
        unsafe { UnixInputStream::new(Self {}) }
    }
}

impl std::os::unix::io::IntoRawFd for Stdin {
    fn into_raw_fd(self) -> std::os::unix::io::RawFd {
        0 as std::os::unix::io::RawFd
    }
}

struct Stdout;

impl Stdout {
    pub fn stream() -> UnixOutputStream {
        unsafe { UnixOutputStream::new(Self {}) }
    }
}

impl std::os::unix::io::IntoRawFd for Stdout {
    fn into_raw_fd(self) -> std::os::unix::io::RawFd {
        1 as std::os::unix::io::RawFd
    }
}

#[derive(Clone, Debug)]
enum Input {
    Stdin,
    Named(PathOrUrl),
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Input::Stdin => "stdin".fmt(f),
            Input::Named(p) => p.fmt(f),
        }
    }
}

#[derive(Clone, Debug)]
enum Output {
    Stdout,
    Path(PathBuf),
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::Stdout => "stdout".fmt(f),
            Output::Path(p) => p.display().fmt(f),
        }
    }
}

arg_enum! {
    #[derive(Clone, Copy, Debug)]
    enum Format {
        Png,
        Pdf,
        Ps,
        Eps,
        Svg,
    }
}

#[derive(Debug)]
struct Converter {
    pub dpi: (f64, f64),
    pub zoom: Scale,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Format,
    pub export_id: Option<String>,
    pub keep_aspect_ratio: bool,
    pub background_color: Option<Color>,
    pub stylesheet: Option<PathBuf>,
    pub unlimited: bool,
    pub keep_image_data: bool,
    pub input: Vec<Input>,
    pub output: Output,
}

impl Converter {
    pub fn convert(self) {
        let stylesheet = match self.stylesheet {
            Some(ref p) => std::fs::read_to_string(p)
                .map(Some)
                .unwrap_or_else(|e| exit!("Error reading stylesheet: {}", e)),
            None => None,
        };

        let mut surface: OnceCell<Surface> = OnceCell::new();

        for input in &self.input {
            let (stream, basefile) = match input {
                Input::Stdin => (Stdin::stream().upcast::<InputStream>(), None),
                Input::Named(p) => {
                    let file = p.get_gfile();
                    let stream = file
                        .read(None::<&Cancellable>)
                        .unwrap_or_else(|e| exit!("Error reading file \"{}\": {}", input, e));
                    (stream.upcast::<InputStream>(), Some(file))
                }
            };

            let mut handle = Loader::new()
                .with_unlimited_size(self.unlimited)
                .keep_image_data(self.keep_image_data)
                .read_stream(&stream, basefile.as_ref(), None::<&Cancellable>)
                .unwrap_or_else(|e| exit!("Error reading SVG {}: {}", input, e));

            if let Some(ref css) = stylesheet {
                handle
                    .set_stylesheet(&css)
                    .unwrap_or_else(|e| exit!("Error applying stylesheet: {}", e));
            }

            let renderer = CairoRenderer::new(&handle).with_dpi(self.dpi.0, self.dpi.1);

            // Create the surface once on the first input
            let s = surface.get_or_init(|| self.create_surface(&renderer, input));

            s.render(
                &renderer,
                self.zoom,
                self.background_color,
                self.export_id.as_deref(),
            )
            .unwrap_or_else(|e| exit!("Error rendering SVG {}: {}", input, e))
        }

        if let Some(s) = surface.take() {
            s.finish()
                .unwrap_or_else(|e| exit!("Error saving output: {}", e))
        };
    }

    fn natural_size(&self, renderer: &CairoRenderer, input: &Input) -> Size {
        let (w, h) = renderer
            .legacy_layer_size(self.export_id.as_deref())
            .unwrap_or_else(|e| match e {
                RenderingError::IdNotFound => exit!(
                    "File {} does not have an object with id \"{}\")",
                    input,
                    self.export_id.as_deref().unwrap()
                ),
                _ => exit!("Error rendering SVG {}: {}", input, e),
            });

        Size::new(w, h)
    }

    fn final_size(&self, strategy: &ResizeStrategy, natural_size: &Size, input: &Input) -> Size {
        strategy
            .apply(
                Size::new(natural_size.w, natural_size.h),
                self.keep_aspect_ratio,
            )
            .unwrap_or_else(|| exit!("The SVG {} has no dimensions", input))
    }

    fn create_surface(&self, renderer: &CairoRenderer, input: &Input) -> Surface {
        let natural_size = self.natural_size(renderer, input);

        let strategy = match (self.width, self.height) {
            // when w and h are not specified, scale to the requested zoom (if any)
            (None, None) => ResizeStrategy::Scale(self.zoom),

            // when w and h are specified, but zoom is not, scale to the requested size
            (Some(w), Some(h)) if self.zoom.is_identity() => ResizeStrategy::Fit(w, h),

            // if only one between w and h is specified and there is no zoom, scale to the
            // requested w or h and use the same scaling factor for the other
            (Some(w), None) if self.zoom.is_identity() => ResizeStrategy::FitWidth(w),
            (None, Some(h)) if self.zoom.is_identity() => ResizeStrategy::FitHeight(h),

            // otherwise scale the image, but cap the zoom to match the requested size
            _ => ResizeStrategy::FitLargestScale(self.zoom, self.width, self.height),
        };

        let final_size = self.final_size(&strategy, &natural_size, input);

        let output_stream = match self.output {
            Output::Stdout => Stdout::stream().upcast::<OutputStream>(),
            Output::Path(ref p) => {
                let file = gio::File::new_for_path(p);
                let stream = file
                    .replace(None, false, FileCreateFlags::NONE, None::<&Cancellable>)
                    .unwrap_or_else(|e| exit!("Error opening output \"{}\": {}", self.output, e));
                stream.upcast::<OutputStream>()
            }
        };

        Surface::new(self.format, final_size, output_stream).unwrap_or_else(|e| match e {
            cairo::Status::InvalidSize => exit!(concat!(
                "The resulting image would be larger than 32767 pixels on either dimension.\n",
                "Librsvg currently cannot render to images bigger than that.\n",
                "Please specify a smaller size."
            )),
            e => exit!("Error creating output surface: {}", e),
        })
    }
}

fn parse_args() -> Result<Converter, clap::Error> {
    let app = clap::App::new("rsvg-convert")
        .version(concat!("version ", crate_version!()))
        .about("Convert SVG files to other image formats")
        .help_short("?")
        .version_short("v")
        .arg(
            clap::Arg::with_name("res_x")
                .short("d")
                .long("dpi-x")
                .takes_value(true)
                .value_name("float")
                .default_value("90")
                .validator(is_valid_resolution)
                .help("Pixels per inch"),
        )
        .arg(
            clap::Arg::with_name("res_y")
                .short("p")
                .long("dpi-y")
                .takes_value(true)
                .value_name("float")
                .default_value("90")
                .validator(is_valid_resolution)
                .help("Pixels per inch"),
        )
        .arg(
            clap::Arg::with_name("zoom_x")
                .short("x")
                .long("x-zoom")
                .takes_value(true)
                .value_name("float")
                .conflicts_with("zoom")
                .validator(is_valid_zoom_factor)
                .help("Horizontal zoom factor"),
        )
        .arg(
            clap::Arg::with_name("zoom_y")
                .short("y")
                .long("y-zoom")
                .takes_value(true)
                .value_name("float")
                .conflicts_with("zoom")
                .validator(is_valid_zoom_factor)
                .help("Vertical zoom factor"),
        )
        .arg(
            clap::Arg::with_name("zoom")
                .short("z")
                .long("zoom")
                .takes_value(true)
                .value_name("float")
                .validator(is_valid_zoom_factor)
                .help("Zoom factor"),
        )
        .arg(
            clap::Arg::with_name("size_x")
                .short("w")
                .long("width")
                .takes_value(true)
                .value_name("pixels")
                .help("Width [defaults to the width of the SVG]"),
        )
        .arg(
            clap::Arg::with_name("size_y")
                .short("h")
                .long("height")
                .takes_value(true)
                .value_name("pixels")
                .help("Height [defaults to the height of the SVG]"),
        )
        .arg(
            clap::Arg::with_name("format")
                .short("f")
                .long("format")
                .takes_value(true)
                .possible_values(&Format::variants())
                .case_insensitive(true)
                .default_value("png")
                .help("Output format"),
        )
        .arg(
            clap::Arg::with_name("output")
                .short("o")
                .long("output")
                .empty_values(false)
                .help("Output filename [defaults to stdout]"),
        )
        .arg(
            clap::Arg::with_name("export_id")
                .short("i")
                .long("export-id")
                .empty_values(false)
                .value_name("object id")
                .help("SVG id of object to export [default is to export all objects]"),
        )
        .arg(
            clap::Arg::with_name("keep_aspect")
                .short("a")
                .long("keep-aspect-ratio")
                .help("Preserve the aspect ratio"),
        )
        .arg(
            clap::Arg::with_name("background")
                .short("b")
                .long("background-color")
                .takes_value(true)
                .value_name("color")
                .help("Set the background color using a CSS color spec"),
        )
        .arg(
            clap::Arg::with_name("stylesheet")
                .short("s")
                .long("stylesheet")
                .empty_values(false)
                .help("Filename of CSS stylesheet to apply"),
        )
        .arg(
            clap::Arg::with_name("unlimited")
                .short("u")
                .long("unlimited")
                .help("Allow huge SVG files"),
        )
        .arg(
            clap::Arg::with_name("keep_image_data")
                .long("keep-image-data")
                .help("Keep image data"),
        )
        .arg(
            clap::Arg::with_name("no_keep_image_data")
                .long("no-keep-image-data")
                .help("Do not keep image data"),
        )
        .arg(
            clap::Arg::with_name("FILE")
                .help("The input file(s) to convert")
                .multiple(true),
        );

    let matches = app.get_matches();

    let format = value_t!(matches, "format", Format)?;

    let keep_image_data = match format {
        Format::Ps | Format::Eps | Format::Pdf => !matches.is_present("no_keep_image_data"),
        _ => matches.is_present("keep_image_data"),
    };

    let background_color = value_t!(matches, "background", String).and_then(parse_color_string);

    // librsvg expects ids starting with '#', so it can lookup ids in externs like "subfile.svg#subid".
    // For the user's convenience, we prepend '#' automatically; we only support specifying ids from
    // the toplevel, and don't expect users to lookup things in externs.
    let lookup_id = |id: String| {
        if id.starts_with('#') {
            id
        } else {
            format!("#{}", id)
        }
    };

    let zoom = value_t!(matches, "zoom", f64).or_none()?;
    let zoom_x = value_t!(matches, "zoom_x", f64).or_none()?;
    let zoom_y = value_t!(matches, "zoom_y", f64).or_none()?;

    let input = match matches.values_of_os("FILE") {
        Some(values) => values
            .map(PathOrUrl::from_os_str)
            .map(Input::Named)
            .collect(),
        None => vec![Input::Stdin],
    };

    if input.len() > 1 && !matches!(format, Format::Ps | Format::Eps | Format::Pdf) {
        return Err(clap::Error::with_description(
            "Multiple SVG files are only allowed for PDF and (E)PS output.",
            clap::ErrorKind::TooManyValues,
        ));
    }

    Ok(Converter {
        dpi: (
            value_t!(matches, "res_x", f64)?,
            value_t!(matches, "res_y", f64)?,
        ),
        zoom: Scale {
            x: zoom.or(zoom_x).unwrap_or(1.0),
            y: zoom.or(zoom_y).unwrap_or(1.0),
        },
        width: value_t!(matches, "size_x", u32).or_none()?,
        height: value_t!(matches, "size_y", u32).or_none()?,
        format,
        export_id: value_t!(matches, "export_id", String)
            .or_none()?
            .map(lookup_id),
        keep_aspect_ratio: matches.is_present("keep_aspect"),
        background_color: background_color.or_none()?,
        stylesheet: matches.value_of_os("stylesheet").map(PathBuf::from),
        unlimited: matches.is_present("unlimited"),
        keep_image_data,
        input,
        output: matches
            .value_of_os("output")
            .map(PathBuf::from)
            .map(Output::Path)
            .unwrap_or(Output::Stdout),
    })
}

fn is_valid_resolution(v: String) -> Result<(), String> {
    match v.parse::<f64>() {
        Ok(res) if res > 0.0 => Ok(()),
        Ok(_) => Err(String::from("Invalid resolution")),
        Err(e) => Err(format!("{}", e)),
    }
}

fn is_valid_zoom_factor(v: String) -> Result<(), String> {
    match v.parse::<f64>() {
        Ok(res) if res > 0.0 => Ok(()),
        Ok(_) => Err(String::from("Invalid zoom factor")),
        Err(e) => Err(format!("{}", e)),
    }
}

trait NotFound {
    type Ok;
    type Error;

    fn or_none(self) -> Result<Option<Self::Ok>, Self::Error>;
}

impl<T> NotFound for Result<T, clap::Error> {
    type Ok = T;
    type Error = clap::Error;

    /// Maps the Result to an Option, translating the ArgumentNotFound error to
    /// Ok(None), while mapping other kinds of errors to Err(e).
    ///
    /// This allows to get proper error reporting for invalid values on optional
    /// arguments.
    fn or_none(self) -> Result<Option<T>, clap::Error> {
        self.map_or_else(
            |e| match e.kind {
                clap::ErrorKind::ArgumentNotFound => Ok(None),
                _ => Err(e),
            },
            |v| Ok(Some(v)),
        )
    }
}

fn parse_color_string<T: AsRef<str> + std::fmt::Display>(s: T) -> Result<Color, clap::Error> {
    match s.as_ref() {
        "none" | "None" => Err(clap::Error::with_description(
            s.as_ref(),
            clap::ErrorKind::ArgumentNotFound,
        )),
        _ => <Color as Parse>::parse_str(s.as_ref()).map_err(|_| {
            let desc = format!(
                "Invalid value: The argument '{}' can not be parsed as a CSS color value",
                s
            );
            clap::Error::with_description(&desc, clap::ErrorKind::InvalidValue)
        }),
    }
}

#[macro_export]
macro_rules! exit {
    () => (exit!("Error"));
    ($($arg:tt)*) => ({
        std::eprintln!("{}", std::format_args!($($arg)*));
        std::process::exit(1);
    })
}

fn main() {
    parse_args().map_or_else(|e| e.exit(), |converter| converter.convert());
}

#[cfg(test)]
mod tests {
    mod color {
        use super::super::*;

        #[test]
        fn valid_color_is_ok() {
            assert!(parse_color_string("Red").is_ok());
        }

        #[test]
        fn none_is_handled_as_not_found() {
            assert_eq!(
                parse_color_string("None").map_err(|e| e.kind),
                Err(clap::ErrorKind::ArgumentNotFound)
            );
        }

        #[test]
        fn invalid_is_handled_as_invalid_value() {
            assert_eq!(
                parse_color_string("foo").map_err(|e| e.kind),
                Err(clap::ErrorKind::InvalidValue)
            );
        }
    }
}
