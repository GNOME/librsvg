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

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<cairo::Status> for Error {
    fn from(s: cairo::Status) -> Self {
        match s {
            cairo::Status::InvalidSize => Self(String::from(
                "The resulting image would be larger than 32767 pixels on either dimension.\n\
                Librsvg currently cannot render to images bigger than that.\n\
                Please specify a smaller size.",
            )),
            e => Self(format!("{}", e)),
        }
    }
}

macro_rules! impl_error_from {
    ($err:ty) => {
        impl From<$err> for Error {
            fn from(e: $err) -> Self {
                Self(format!("{}", e))
            }
        }
    };
}

impl_error_from!(RenderingError);
impl_error_from!(cairo::IoError);
impl_error_from!(cairo::StreamWithError);
impl_error_from!(clap::Error);

macro_rules! error {
    ($($arg:tt)*) => (Error(std::format!($($arg)*)));
}

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
    #[cfg(have_cairo_pdf)]
    Pdf(cairo::PdfSurface, Size),
    #[cfg(have_cairo_ps)]
    Ps(cairo::PsSurface, Size),
    #[cfg(have_cairo_svg)]
    Svg(cairo::SvgSurface, Size),
}

impl Deref for Surface {
    type Target = cairo::Surface;

    fn deref(&self) -> &cairo::Surface {
        match self {
            Self::Png(surface, _) => &surface,
            #[cfg(have_cairo_pdf)]
            Self::Pdf(surface, _) => &surface,
            #[cfg(have_cairo_ps)]
            Self::Ps(surface, _) => &surface,
            #[cfg(have_cairo_svg)]
            Self::Svg(surface, _) => &surface,
        }
    }
}

impl Surface {
    pub fn new(format: Format, size: Size, stream: OutputStream) -> Result<Self, Error> {
        match format {
            Format::Png => Self::new_for_png(size, stream),
            Format::Pdf => Self::new_for_pdf(size, stream),
            Format::Ps => Self::new_for_ps(size, stream, false),
            Format::Eps => Self::new_for_ps(size, stream, true),
            Format::Svg => Self::new_for_svg(size, stream),
        }
    }

    fn new_for_png(size: Size, stream: OutputStream) -> Result<Self, Error> {
        let w = checked_i32(size.w.round())?;
        let h = checked_i32(size.h.round())?;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
        Ok(Self::Png(surface, stream))
    }

    #[cfg(have_cairo_pdf)]
    fn new_for_pdf(size: Size, stream: OutputStream) -> Result<Self, Error> {
        let surface = cairo::PdfSurface::for_stream(size.w, size.h, stream.into_write())?;
        if let Some(date) = metadata::creation_date()? {
            surface.set_metadata(cairo::PdfMetadata::CreateDate, &date)?;
        }
        Ok(Self::Pdf(surface, size))
    }

    #[cfg(not(have_cairo_pdf))]
    fn new_for_pdf(_size: Size, _stream: OutputStream) -> Result<Self, Error> {
        Err(Error("unsupported format".to_string()))
    }

    #[cfg(have_cairo_ps)]
    fn new_for_ps(size: Size, stream: OutputStream, eps: bool) -> Result<Self, Error> {
        let surface = cairo::PsSurface::for_stream(size.w, size.h, stream.into_write())?;
        surface.set_eps(eps);
        Ok(Self::Ps(surface, size))
    }

    #[cfg(not(have_cairo_ps))]
    fn new_for_ps(_size: Size, _stream: OutputStream, _eps: bool) -> Result<Self, Error> {
        Err(Error("unsupported format".to_string()))
    }

    #[cfg(have_cairo_svg)]
    fn new_for_svg(size: Size, stream: OutputStream) -> Result<Self, Error> {
        let surface = cairo::SvgSurface::for_stream(size.w, size.h, stream.into_write())?;
        Ok(Self::Svg(surface, size))
    }

    #[cfg(not(have_cairo_svg))]
    fn new_for_svg(_size: Size, _stream: OutputStream) -> Result<Self, Error> {
        Err(Error("unsupported format".to_string()))
    }

    pub fn render(
        &self,
        renderer: &CairoRenderer,
        final_size: Size,
        geometry: cairo::Rectangle,
        background_color: Option<Color>,
        id: Option<&str>,
    ) -> Result<(), Error> {
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

        // Note that we don't scale the viewport; we change the cr's transform instead.  This
        // is because SVGs are rendered proportionally to fit within the viewport, regardless
        // of the viewport's proportions.  Rsvg-convert allows non-proportional scaling, so
        // we do that with a separate transform.

        let scale = Scale {
            x: final_size.w / geometry.width,
            y: final_size.h / geometry.height,
        };

        cr.scale(scale.x, scale.y);

        let viewport = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: geometry.width,
            height: geometry.height,
        };

        match id {
            None => renderer.render_document(&cr, &viewport)?,
            Some(_) => renderer.render_element(&cr, id, &viewport)?,
        }

        if !matches!(self, Self::Png(_, _)) {
            cr.show_page();
        }

        Ok(())
    }

    pub fn finish(self) -> Result<(), Error> {
        match self {
            Self::Png(surface, stream) => surface.write_to_png(&mut stream.into_write())?,
            _ => self.finish_output_stream().map(|_| ())?,
        }

        Ok(())
    }
}

fn checked_i32(x: f64) -> Result<i32, cairo::Status> {
    cast::i32(x).map_err(|_| cairo::Status::InvalidSize)
}

mod metadata {
    use crate::Error;
    use chrono::prelude::*;
    use std::env;
    use std::str::FromStr;

    pub fn creation_date() -> Result<Option<String>, Error> {
        match env::var("SOURCE_DATE_EPOCH") {
            Ok(epoch) => match i64::from_str(&epoch) {
                Ok(seconds) => {
                    let datetime = Utc.timestamp(seconds, 0);
                    Ok(Some(datetime.to_rfc3339()))
                }
                Err(e) => Err(error!("Environment variable $SOURCE_DATE_EPOCH: {}", e)),
            },
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => Err(error!(
                "Environment variable $SOURCE_DATE_EPOCH is not valid Unicode"
            )),
        }
    }
}

struct Stdin;

impl Stdin {
    pub fn stream() -> UnixInputStream {
        unsafe { UnixInputStream::new(0) }
    }
}

struct Stdout;

impl Stdout {
    pub fn stream() -> UnixOutputStream {
        unsafe { UnixOutputStream::new(1) }
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
    // Keep this enum in sync with supported_formats in parse_args()
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
    pub fn convert(self) -> Result<(), Error> {
        let stylesheet = match self.stylesheet {
            Some(ref p) => std::fs::read_to_string(p)
                .map(Some)
                .map_err(|e| error!("Error reading stylesheet: {}", e))?,
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
                        .map_err(|e| error!("Error reading file \"{}\": {}", input, e))?;
                    (stream.upcast::<InputStream>(), Some(file))
                }
            };

            let mut handle = Loader::new()
                .with_unlimited_size(self.unlimited)
                .keep_image_data(self.keep_image_data)
                .read_stream(&stream, basefile.as_ref(), None::<&Cancellable>)
                .map_err(|e| error!("Error reading SVG {}: {}", input, e))?;

            if let Some(ref css) = stylesheet {
                handle
                    .set_stylesheet(&css)
                    .map_err(|e| error!("Error applying stylesheet: {}", e))?;
            }

            let renderer = CairoRenderer::new(&handle).with_dpi(self.dpi.0, self.dpi.1);

            let geometry = self.natural_geometry(&renderer, input)?;
            let natural_size = Size::new(geometry.width, geometry.height);

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

            let final_size = self.final_size(&strategy, &natural_size, input)?;

            // Create the surface once on the first input
            let s = surface.get_or_try_init(|| self.create_surface(final_size))?;

            s.render(
                &renderer,
                final_size,
                geometry,
                self.background_color,
                self.export_id.as_deref(),
            )
            .map_err(|e| error!("Error rendering SVG {}: {}", input, e))?
        }

        if let Some(s) = surface.take() {
            s.finish()
                .map_err(|e| error!("Error saving output {}: {}", self.output, e))?
        };

        Ok(())
    }

    fn natural_geometry(
        &self,
        renderer: &CairoRenderer,
        input: &Input,
    ) -> Result<cairo::Rectangle, Error> {
        match self.export_id {
            None => renderer.legacy_layer_geometry(None),
            Some(ref id) => renderer.geometry_for_element(Some(&id)),
        }
        .map(|(ink_r, _)| ink_r)
        .map_err(|e| match e {
            RenderingError::IdNotFound => error!(
                "File {} does not have an object with id \"{}\")",
                input,
                self.export_id.as_deref().unwrap()
            ),
            _ => error!("Error rendering SVG {}: {}", input, e),
        })
    }

    fn final_size(
        &self,
        strategy: &ResizeStrategy,
        natural_size: &Size,
        input: &Input,
    ) -> Result<Size, Error> {
        strategy
            .apply(
                Size::new(natural_size.w, natural_size.h),
                self.keep_aspect_ratio,
            )
            .ok_or_else(|| error!("The SVG {} has no dimensions", input))
    }

    fn create_surface(&self, size: Size) -> Result<Surface, Error> {
        let output_stream = match self.output {
            Output::Stdout => Stdout::stream().upcast::<OutputStream>(),
            Output::Path(ref p) => {
                let file = gio::File::new_for_path(p);
                let stream = file
                    .replace(None, false, FileCreateFlags::NONE, None::<&Cancellable>)
                    .map_err(|e| error!("Error opening output \"{}\": {}", self.output, e))?;
                stream.upcast::<OutputStream>()
            }
        };

        Surface::new(self.format, size, output_stream)
    }
}

fn parse_args() -> Result<Converter, Error> {
    let supported_formats = vec![
        "Png",
        #[cfg(have_cairo_pdf)]
        "Pdf",
        #[cfg(have_cairo_ps)]
        "Ps",
        #[cfg(have_cairo_ps)]
        "Eps",
        #[cfg(have_cairo_svg)]
        "Svg",
    ];

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
                .default_value("96")
                .validator(is_valid_resolution)
                .help("Pixels per inch"),
        )
        .arg(
            clap::Arg::with_name("res_y")
                .short("p")
                .long("dpi-y")
                .takes_value(true)
                .value_name("float")
                .default_value("96")
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
                .possible_values(supported_formats.as_slice())
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
            .map(|f| PathOrUrl::from_os_str(f).map_err(Error))
            .map(|r| r.map(Input::Named))
            .collect::<Result<Vec<Input>, Error>>()?,

        None => vec![Input::Stdin],
    };

    if input.len() > 1 && !matches!(format, Format::Ps | Format::Eps | Format::Pdf) {
        return Err(error!(
            "Multiple SVG files are only allowed for PDF and (E)PS output."
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

fn main() {
    if let Err(e) = parse_args().and_then(|converter| converter.convert()) {
        std::eprintln!("{}", e);
        std::process::exit(1);
    }
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
