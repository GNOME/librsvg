use clap::{arg_enum, crate_version, value_t};

use gio::prelude::*;
use gio::{Cancellable, FileCreateFlags, InputStream, OutputStream};

#[cfg(unix)]
use gio::{UnixInputStream, UnixOutputStream};

#[cfg(windows)]
mod windows_imports {
    pub use gio::{Win32InputStream, WriteOutputStream};
    pub use glib::ffi::gboolean;
    pub use glib::translate::*;
    pub use libc::c_void;
    pub use std::io;
    pub use std::os::windows::io::AsRawHandle;
}
#[cfg(windows)]
use self::windows_imports::*;

use librsvg::rsvg_convert_only::{
    AspectRatio, CssLength, Dpi, Horizontal, LegacySize, Length, Normalize, NormalizeParams, Parse,
    PathOrUrl, Rect, ULength, Validate, Vertical, ViewBox,
};
use librsvg::{AcceptLanguage, CairoRenderer, Color, Language, LengthUnit, Loader, RenderingError};
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<cairo::Error> for Error {
    fn from(s: cairo::Error) -> Self {
        match s {
            cairo::Error::InvalidSize => Self(String::from(
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

#[derive(Clone, Copy, Debug, PartialEq)]
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
    Fit {
        size: Size,
        keep_aspect_ratio: bool,
    },
    FitWidth(f64),
    FitHeight(f64),
    ScaleWithMaxSize {
        scale: Scale,
        max_width: Option<f64>,
        max_height: Option<f64>,
        keep_aspect_ratio: bool,
    },
}

impl ResizeStrategy {
    pub fn apply(self, input: &Size) -> Option<Size> {
        if input.w == 0.0 || input.h == 0.0 {
            return None;
        }

        let output_size = match self {
            ResizeStrategy::Scale(s) => Size::new(input.w * s.x, input.h * s.y),

            ResizeStrategy::Fit {
                size,
                keep_aspect_ratio,
            } => {
                if keep_aspect_ratio {
                    let aspect = AspectRatio::parse_str("xMinYMin meet").unwrap();
                    let rect = aspect.compute(
                        &ViewBox::from(Rect::from_size(input.w, input.h)),
                        &Rect::from_size(size.w, size.h),
                    );
                    Size::new(rect.width(), rect.height())
                } else {
                    size
                }
            }

            ResizeStrategy::FitWidth(w) => Size::new(w, input.h * w / input.w),

            ResizeStrategy::FitHeight(h) => Size::new(input.w * h / input.h, h),

            ResizeStrategy::ScaleWithMaxSize {
                scale,
                max_width,
                max_height,
                keep_aspect_ratio,
            } => {
                let scaled = Size::new(input.w * scale.x, input.h * scale.y);

                match (max_width, max_height, keep_aspect_ratio) {
                    (None, None, _) => scaled,

                    (Some(max_width), Some(max_height), false) => {
                        if scaled.w <= max_width && scaled.h <= max_height {
                            scaled
                        } else {
                            Size::new(max_width, max_height)
                        }
                    }

                    (Some(max_width), Some(max_height), true) => {
                        if scaled.w <= max_width && scaled.h <= max_height {
                            scaled
                        } else {
                            let aspect = AspectRatio::parse_str("xMinYMin meet").unwrap();
                            let rect = aspect.compute(
                                &ViewBox::from(Rect::from_size(scaled.w, scaled.h)),
                                &Rect::from_size(max_width, max_height),
                            );
                            Size::new(rect.width(), rect.height())
                        }
                    }

                    (Some(max_width), None, false) => {
                        if scaled.w <= max_width {
                            scaled
                        } else {
                            Size::new(max_width, scaled.h)
                        }
                    }

                    (Some(max_width), None, true) => {
                        if scaled.w <= max_width {
                            scaled
                        } else {
                            let factor = max_width / scaled.w;
                            Size::new(max_width, scaled.h * factor)
                        }
                    }

                    (None, Some(max_height), false) => {
                        if scaled.h <= max_height {
                            scaled
                        } else {
                            Size::new(scaled.w, max_height)
                        }
                    }

                    (None, Some(max_height), true) => {
                        if scaled.h <= max_height {
                            scaled
                        } else {
                            let factor = max_height / scaled.h;
                            Size::new(scaled.w * factor, max_height)
                        }
                    }
                }
            }
        };

        Some(output_size)
    }
}

enum Surface {
    Png(cairo::ImageSurface, OutputStream),
    #[cfg(system_deps_have_cairo_pdf)]
    Pdf(cairo::PdfSurface, Size),
    #[cfg(system_deps_have_cairo_ps)]
    Ps(cairo::PsSurface, Size),
    #[cfg(system_deps_have_cairo_svg)]
    Svg(cairo::SvgSurface, Size),
}

impl Deref for Surface {
    type Target = cairo::Surface;

    fn deref(&self) -> &cairo::Surface {
        match self {
            Self::Png(surface, _) => surface,
            #[cfg(system_deps_have_cairo_pdf)]
            Self::Pdf(surface, _) => surface,
            #[cfg(system_deps_have_cairo_ps)]
            Self::Ps(surface, _) => surface,
            #[cfg(system_deps_have_cairo_svg)]
            Self::Svg(surface, _) => surface,
        }
    }
}

impl Surface {
    pub fn new(
        format: Format,
        size: Size,
        stream: OutputStream,
        unit: LengthUnit,
    ) -> Result<Self, Error> {
        match format {
            Format::Png => Self::new_for_png(size, stream),
            Format::Pdf => Self::new_for_pdf(size, stream),
            Format::Ps => Self::new_for_ps(size, stream, false),
            Format::Eps => Self::new_for_ps(size, stream, true),
            Format::Svg => Self::new_for_svg(size, stream, unit),
        }
    }

    fn new_for_png(size: Size, stream: OutputStream) -> Result<Self, Error> {
        // We use ceil() to avoid chopping off the last pixel if it is partially covered.
        let w = checked_i32(size.w.ceil())?;
        let h = checked_i32(size.h.ceil())?;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
        Ok(Self::Png(surface, stream))
    }

    #[cfg(system_deps_have_cairo_pdf)]
    fn new_for_pdf(size: Size, stream: OutputStream) -> Result<Self, Error> {
        let surface = cairo::PdfSurface::for_stream(size.w, size.h, stream.into_write())?;
        if let Some(date) = metadata::creation_date()? {
            surface.set_metadata(cairo::PdfMetadata::CreateDate, &date)?;
        }
        Ok(Self::Pdf(surface, size))
    }

    #[cfg(not(system_deps_have_cairo_pdf))]
    fn new_for_pdf(_size: Size, _stream: OutputStream) -> Result<Self, Error> {
        Err(Error("unsupported format".to_string()))
    }

    #[cfg(system_deps_have_cairo_ps)]
    fn new_for_ps(size: Size, stream: OutputStream, eps: bool) -> Result<Self, Error> {
        let surface = cairo::PsSurface::for_stream(size.w, size.h, stream.into_write())?;
        surface.set_eps(eps);
        Ok(Self::Ps(surface, size))
    }

    #[cfg(not(system_deps_have_cairo_ps))]
    fn new_for_ps(_size: Size, _stream: OutputStream, _eps: bool) -> Result<Self, Error> {
        Err(Error("unsupported format".to_string()))
    }

    #[cfg(system_deps_have_cairo_svg)]
    fn new_for_svg(size: Size, stream: OutputStream, unit: LengthUnit) -> Result<Self, Error> {
        let mut surface = cairo::SvgSurface::for_stream(size.w, size.h, stream.into_write())?;

        let svg_unit = match unit {
            LengthUnit::Cm => cairo::SvgUnit::Cm,
            LengthUnit::In => cairo::SvgUnit::In,
            LengthUnit::Mm => cairo::SvgUnit::Mm,
            LengthUnit::Pc => cairo::SvgUnit::Pc,
            LengthUnit::Pt => cairo::SvgUnit::Pt,
            _ => cairo::SvgUnit::User,
        };

        surface.set_document_unit(svg_unit);
        Ok(Self::Svg(surface, size))
    }

    #[cfg(not(system_deps_have_cairo_svg))]
    fn new_for_svg(_size: Size, _stream: OutputStream, u: LengthUnit) -> Result<Self, Error> {
        Err(Error("unsupported format".to_string()))
    }

    #[allow(clippy::too_many_arguments)] // yeah, yeah, we'll refactor it eventually
    pub fn render(
        &self,
        renderer: &CairoRenderer,
        left: f64,
        top: f64,
        final_size: Size,
        geometry: cairo::Rectangle,
        background_color: Option<Color>,
        id: Option<&str>,
    ) -> Result<(), Error> {
        let cr = cairo::Context::new(self)?;

        if let Some(Color::RGBA(rgba)) = background_color {
            cr.set_source_rgba(
                rgba.red_f32().into(),
                rgba.green_f32().into(),
                rgba.blue_f32().into(),
                rgba.alpha_f32().into(),
            );

            cr.paint()?;
        }

        cr.translate(left, top);

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
            cr.show_page()?;
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

fn checked_i32(x: f64) -> Result<i32, cairo::Error> {
    cast::i32(x).map_err(|_| cairo::Error::InvalidSize)
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
    #[cfg(unix)]
    pub fn stream() -> InputStream {
        let stream = unsafe { UnixInputStream::with_fd(0) };
        stream.upcast::<InputStream>()
    }

    #[cfg(windows)]
    pub fn stream() -> InputStream {
        let stream = unsafe { Win32InputStream::with_handle(io::stdin()) };
        stream.upcast::<InputStream>()
    }
}

struct Stdout;

impl Stdout {
    #[cfg(unix)]
    pub fn stream() -> OutputStream {
        let stream = unsafe { UnixOutputStream::with_fd(1) };
        stream.upcast::<OutputStream>()
    }

    #[cfg(windows)]
    pub fn stream() -> OutputStream {
        // Ideally, we could use a Win32OutputStream, but when it's used with a file redirect,
        // it gets buggy.
        // https://gitlab.gnome.org/GNOME/librsvg/-/issues/812
        let stream = WriteOutputStream::new(io::stdout());
        stream.upcast::<OutputStream>()
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

struct Converter {
    pub dpi: (f64, f64),
    pub zoom: Scale,
    pub width: Option<ULength<Horizontal>>,
    pub height: Option<ULength<Vertical>>,
    pub left: Option<Length<Horizontal>>,
    pub top: Option<Length<Vertical>>,
    pub page_size: Option<(ULength<Horizontal>, ULength<Vertical>)>,
    pub format: Format,
    pub export_id: Option<String>,
    pub keep_aspect_ratio: bool,
    pub background_color: Option<Color>,
    pub stylesheet: Option<PathBuf>,
    pub language: Language,
    pub unlimited: bool,
    pub keep_image_data: bool,
    pub input: Vec<Input>,
    pub output: Output,
    pub testing: bool,
}

impl Converter {
    pub fn convert(self) -> Result<(), Error> {
        let stylesheet = match self.stylesheet {
            Some(ref p) => std::fs::read_to_string(p)
                .map(Some)
                .map_err(|e| error!("Error reading stylesheet: {}", e))?,
            None => None,
        };

        let mut surface: Option<Surface> = None;

        // Use user units per default
        let mut unit = LengthUnit::Px;

        fn set_unit<N: Normalize, V: Validate>(
            l: CssLength<N, V>,
            p: &NormalizeParams,
            u: LengthUnit,
        ) -> f64 {
            match u {
                LengthUnit::Pt => l.to_points(p),
                LengthUnit::In => l.to_inches(p),
                LengthUnit::Cm => l.to_cm(p),
                LengthUnit::Mm => l.to_mm(p),
                LengthUnit::Pc => l.to_picas(p),
                _ => l.to_user(p),
            }
        }

        for (page_idx, input) in self.input.iter().enumerate() {
            let (stream, basefile) = match input {
                Input::Stdin => (Stdin::stream(), None),
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
                    .set_stylesheet(css)
                    .map_err(|e| error!("Error applying stylesheet: {}", e))?;
            }

            let renderer = CairoRenderer::new(&handle)
                .with_dpi(self.dpi.0, self.dpi.1)
                .with_language(&self.language)
                .test_mode(self.testing);

            let geometry = natural_geometry(&renderer, input, self.export_id.as_deref())?;

            let natural_size = Size::new(geometry.width, geometry.height);

            let params = NormalizeParams::from_dpi(Dpi::new(self.dpi.0, self.dpi.1));

            // Convert natural size and requested size to pixels or points, depending on the target format,
            let (natural_size, requested_width, requested_height, page_size) = match self.format {
                Format::Png => {
                    // PNG surface requires units in pixels
                    (
                        natural_size,
                        self.width.map(|l| l.to_user(&params)),
                        self.height.map(|l| l.to_user(&params)),
                        self.page_size.map(|(w, h)| Size {
                            w: w.to_user(&params),
                            h: h.to_user(&params),
                        }),
                    )
                }

                Format::Pdf | Format::Ps | Format::Eps => {
                    // These surfaces require units in points
                    unit = LengthUnit::Pt;

                    (
                        Size {
                            w: ULength::<Horizontal>::new(natural_size.w, LengthUnit::Px)
                                .to_points(&params),
                            h: ULength::<Vertical>::new(natural_size.h, LengthUnit::Px)
                                .to_points(&params),
                        },
                        self.width.map(|l| l.to_points(&params)),
                        self.height.map(|l| l.to_points(&params)),
                        self.page_size.map(|(w, h)| Size {
                            w: w.to_points(&params),
                            h: h.to_points(&params),
                        }),
                    )
                }

                Format::Svg => {
                    let (w_unit, h_unit) =
                        (self.width.map(|l| l.unit), self.height.map(|l| l.unit));

                    unit = match (w_unit, h_unit) {
                        (None, None) => LengthUnit::Px,
                        (None, u) | (u, None) => u.unwrap(),
                        (u1, u2) => {
                            if u1 == u2 {
                                u1.unwrap()
                            } else {
                                LengthUnit::Px
                            }
                        }
                    };

                    // Supported SVG units are px, in, cm, mm, pt, pc
                    (
                        Size {
                            w: set_unit(
                                ULength::<Horizontal>::new(natural_size.w, LengthUnit::Px),
                                &params,
                                unit,
                            ),
                            h: set_unit(
                                ULength::<Vertical>::new(natural_size.h, LengthUnit::Px),
                                &params,
                                unit,
                            ),
                        },
                        self.width.map(|l| set_unit(l, &params, unit)),
                        self.height.map(|l| set_unit(l, &params, unit)),
                        self.page_size.map(|(w, h)| Size {
                            w: set_unit(w, &params, unit),
                            h: set_unit(h, &params, unit),
                        }),
                    )
                }
            };

            let strategy = match (requested_width, requested_height) {
                // when w and h are not specified, scale to the requested zoom (if any)
                (None, None) => ResizeStrategy::Scale(self.zoom),

                // when w and h are specified, but zoom is not, scale to the requested size
                (Some(width), Some(height)) if self.zoom.is_identity() => ResizeStrategy::Fit {
                    size: Size::new(width, height),
                    keep_aspect_ratio: self.keep_aspect_ratio,
                },

                // if only one between w and h is specified and there is no zoom, scale to the
                // requested w or h and use the same scaling factor for the other
                (Some(w), None) if self.zoom.is_identity() => ResizeStrategy::FitWidth(w),
                (None, Some(h)) if self.zoom.is_identity() => ResizeStrategy::FitHeight(h),

                // otherwise scale the image, but cap the zoom to match the requested size
                _ => ResizeStrategy::ScaleWithMaxSize {
                    scale: self.zoom,
                    max_width: requested_width,
                    max_height: requested_height,
                    keep_aspect_ratio: self.keep_aspect_ratio,
                },
            };

            let final_size = self.final_size(&strategy, &natural_size, input)?;

            // Create the surface once on the first input,
            // except for PDF, PS, and EPS, which allow differently-sized pages.
            let page_size = page_size.unwrap_or(final_size);
            let s = match &mut surface {
                Some(s) => {
                    match s {
                        #[cfg(system_deps_have_cairo_pdf)]
                        Surface::Pdf(pdf, size) => {
                            pdf.set_size(page_size.w, page_size.h).map_err(|e| {
                                error!(
                                    "Error setting PDF page #{} size {}: {}",
                                    page_idx + 1,
                                    input,
                                    e
                                )
                            })?;
                            *size = page_size;
                        }
                        #[cfg(system_deps_have_cairo_ps)]
                        Surface::Ps(ps, size) => {
                            ps.set_size(page_size.w, page_size.h);
                            *size = page_size;
                        }
                        _ => {}
                    }
                    s
                }
                surface @ None => surface.insert(self.create_surface(page_size, unit)?),
            };

            let left = self.left.map(|l| set_unit(l, &params, unit)).unwrap_or(0.0);
            let top = self.top.map(|l| set_unit(l, &params, unit)).unwrap_or(0.0);

            s.render(
                &renderer,
                left,
                top,
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

    fn final_size(
        &self,
        strategy: &ResizeStrategy,
        natural_size: &Size,
        input: &Input,
    ) -> Result<Size, Error> {
        strategy
            .apply(natural_size)
            .ok_or_else(|| error!("The SVG {} has no dimensions", input))
    }

    fn create_surface(&self, size: Size, unit: LengthUnit) -> Result<Surface, Error> {
        let output_stream = match self.output {
            Output::Stdout => Stdout::stream(),
            Output::Path(ref p) => {
                let file = gio::File::for_path(p);
                let stream = file
                    .replace(None, false, FileCreateFlags::NONE, None::<&Cancellable>)
                    .map_err(|e| error!("Error opening output \"{}\": {}", self.output, e))?;
                stream.upcast::<OutputStream>()
            }
        };

        Surface::new(self.format, size, output_stream, unit)
    }
}

fn natural_geometry(
    renderer: &CairoRenderer,
    input: &Input,
    export_id: Option<&str>,
) -> Result<cairo::Rectangle, Error> {
    match export_id {
        None => renderer.legacy_layer_geometry(None),
        Some(id) => renderer.geometry_for_element(Some(id)),
    }
    .map(|(ink_r, _)| ink_r)
    .map_err(|e| match e {
        RenderingError::IdNotFound => error!(
            "File {} does not have an object with id \"{}\")",
            input,
            export_id.unwrap()
        ),
        _ => error!("Error rendering SVG {}: {}", input, e),
    })
}

fn parse_args() -> Result<Converter, Error> {
    let supported_formats = vec![
        "Png",
        #[cfg(system_deps_have_cairo_pdf)]
        "Pdf",
        #[cfg(system_deps_have_cairo_ps)]
        "Ps",
        #[cfg(system_deps_have_cairo_ps)]
        "Eps",
        #[cfg(system_deps_have_cairo_svg)]
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
                .value_name("number")
                .default_value("96")
                .validator(is_valid_resolution)
                .help("Pixels per inch"),
        )
        .arg(
            clap::Arg::with_name("res_y")
                .short("p")
                .long("dpi-y")
                .takes_value(true)
                .value_name("number")
                .default_value("96")
                .validator(is_valid_resolution)
                .help("Pixels per inch"),
        )
        .arg(
            clap::Arg::with_name("zoom_x")
                .short("x")
                .long("x-zoom")
                .takes_value(true)
                .value_name("number")
                .conflicts_with("zoom")
                .validator(is_valid_zoom_factor)
                .help("Horizontal zoom factor"),
        )
        .arg(
            clap::Arg::with_name("zoom_y")
                .short("y")
                .long("y-zoom")
                .takes_value(true)
                .value_name("number")
                .conflicts_with("zoom")
                .validator(is_valid_zoom_factor)
                .help("Vertical zoom factor"),
        )
        .arg(
            clap::Arg::with_name("zoom")
                .short("z")
                .long("zoom")
                .takes_value(true)
                .value_name("number")
                .validator(is_valid_zoom_factor)
                .help("Zoom factor"),
        )
        .arg(
            clap::Arg::with_name("size_x")
                .short("w")
                .long("width")
                .takes_value(true)
                .value_name("length")
                .help("Width [defaults to the width of the SVG]"),
        )
        .arg(
            clap::Arg::with_name("size_y")
                .short("h")
                .long("height")
                .takes_value(true)
                .value_name("length")
                .help("Height [defaults to the height of the SVG]"),
        )
        .arg(
            clap::Arg::with_name("top")
                .long("top")
                .takes_value(true)
                .value_name("length")
                .help("Distance between top edge of page and the image [defaults to 0]"),
        )
        .arg(
            clap::Arg::with_name("left")
                .long("left")
                .takes_value(true)
                .value_name("length")
                .help("Distance between left edge of page and the image [defaults to 0]"),
        )
        .arg(
            clap::Arg::with_name("page_width")
                .long("page-width")
                .takes_value(true)
                .value_name("length")
                .help("Width of output media [defaults to the width of the SVG]"),
        )
        .arg(
            clap::Arg::with_name("page_height")
                .long("page-height")
                .takes_value(true)
                .value_name("length")
                .help("Height of output media [defaults to the height of the SVG]"),
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
            clap::Arg::with_name("accept-language")
                .short("l")
                .long("accept-language")
                .empty_values(false)
                .value_name("languages")
                .help("Languages to accept, for example \"es-MX,de,en\" [default uses language from the environment]"),
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
                .value_name("filename.css")
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
            clap::Arg::with_name("testing")
                .long("testing")
                .help("Render images for librsvg's test suite")
                .hidden(true),
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

    let language = value_t!(matches, "accept-language", String)
        .or_none()
        .and_then(|lang_str| match lang_str {
            None => Ok(Language::FromEnvironment),
            Some(s) => AcceptLanguage::parse(&s)
                .map(Language::AcceptLanguage)
                .map_err(|e| {
                    let desc = format!("{}", e);
                    clap::Error::with_description(&desc, clap::ErrorKind::InvalidValue)
                }),
        });

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

    let width = value_t!(matches, "size_x", String)
        .or_none()?
        .map(parse_length)
        .transpose()?;
    let height = value_t!(matches, "size_y", String)
        .or_none()?
        .map(parse_length)
        .transpose()?;

    let left = value_t!(matches, "left", String)
        .or_none()?
        .map(parse_length)
        .transpose()?;
    let top = value_t!(matches, "top", String)
        .or_none()?
        .map(parse_length)
        .transpose()?;

    let page_width = value_t!(matches, "page_width", String)
        .or_none()?
        .map(parse_length)
        .transpose()?;
    let page_height = value_t!(matches, "page_height", String)
        .or_none()?
        .map(parse_length)
        .transpose()?;

    let page_size = match (page_width, page_height) {
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            return Err(error!(
                "Please specify both the --page-width and --page-height options together."
            ));
        }
        (Some(w), Some(h)) => Some((w, h)),
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
        width,
        height,
        left,
        top,
        page_size,
        format,
        export_id: value_t!(matches, "export_id", String)
            .or_none()?
            .map(lookup_id),
        keep_aspect_ratio: matches.is_present("keep_aspect"),
        background_color: background_color.or_none()?,
        stylesheet: matches.value_of_os("stylesheet").map(PathBuf::from),
        unlimited: matches.is_present("unlimited"),
        keep_image_data,
        language: language?,
        input,
        output: matches
            .value_of_os("output")
            .map(PathBuf::from)
            .map(Output::Path)
            .unwrap_or(Output::Stdout),
        testing: matches.is_present("testing"),
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

fn is_absolute_unit(u: LengthUnit) -> bool {
    use LengthUnit::*;

    match u {
        Percent | Em | Ex => false,
        Px | In | Cm | Mm | Pt | Pc => true,
    }
}

fn parse_length<N: Normalize, V: Validate>(s: String) -> Result<CssLength<N, V>, clap::Error> {
    <CssLength<N, V> as Parse>::parse_str(&s)
        .map_err(|_| {
            let desc = format!(
                "Invalid value: The argument '{}' can not be parsed as a length",
                s
            );
            clap::Error::with_description(&desc, clap::ErrorKind::InvalidValue)
        })
        .and_then(|l| {
            if is_absolute_unit(l.unit) {
                Ok(l)
            } else {
                let desc = format!(
                    "Invalid value '{}': supported units are px, in, cm, mm, pt, pc",
                    s
                );
                Err(clap::Error::with_description(
                    &desc,
                    clap::ErrorKind::InvalidValue,
                ))
            }
        })
}

fn main() {
    if let Err(e) = parse_args().and_then(|converter| converter.convert()) {
        std::eprintln!("{}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;

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

#[cfg(test)]
mod sizing_tests {
    use super::*;

    #[test]
    fn detects_empty_size() {
        let strategy = ResizeStrategy::Scale(Scale { x: 42.0, y: 42.0 });
        assert!(strategy.apply(&Size::new(0.0, 0.0)).is_none());
    }

    #[test]
    fn scale() {
        let strategy = ResizeStrategy::Scale(Scale { x: 2.0, y: 3.0 });
        assert_eq!(
            strategy.apply(&Size::new(1.0, 2.0)).unwrap(),
            Size::new(2.0, 6.0),
        );
    }

    #[test]
    fn fit_non_proportional() {
        let strategy = ResizeStrategy::Fit {
            size: Size::new(40.0, 10.0),
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(2.0, 1.0)).unwrap(),
            Size::new(40.0, 10.0),
        );
    }

    #[test]
    fn fit_proportional_wider_than_tall() {
        let strategy = ResizeStrategy::Fit {
            size: Size::new(40.0, 10.0),
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(2.0, 1.0)).unwrap(),
            Size::new(20.0, 10.0),
        );
    }

    #[test]
    fn fit_proportional_taller_than_wide() {
        let strategy = ResizeStrategy::Fit {
            size: Size::new(100.0, 50.0),
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(1.0, 2.0)).unwrap(),
            Size::new(25.0, 50.0),
        );
    }

    #[test]
    fn fit_width() {
        let strategy = ResizeStrategy::FitWidth(100.0);

        assert_eq!(
            strategy.apply(&Size::new(1.0, 2.0)).unwrap(),
            Size::new(100.0, 200.0),
        );
    }

    #[test]
    fn fit_height() {
        let strategy = ResizeStrategy::FitHeight(100.0);

        assert_eq!(
            strategy.apply(&Size::new(1.0, 2.0)).unwrap(),
            Size::new(50.0, 100.0),
        );
    }

    #[test]
    fn scale_no_max_size_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 2.0, y: 3.0 },
            max_width: None,
            max_height: None,
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(1.0, 2.0)).unwrap(),
            Size::new(2.0, 6.0),
        );
    }

    #[test]
    fn scale_with_max_width_and_height_fits_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 2.0, y: 3.0 },
            max_width: Some(10.0),
            max_height: Some(20.0),
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(4.0, 2.0)).unwrap(),
            Size::new(8.0, 6.0)
        );
    }

    #[test]
    fn scale_with_max_width_and_height_fits_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 2.0, y: 3.0 },
            max_width: Some(10.0),
            max_height: Some(20.0),
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(4.0, 2.0)).unwrap(),
            Size::new(8.0, 6.0)
        );
    }

    #[test]
    fn scale_with_max_width_and_height_doesnt_fit_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 10.0, y: 20.0 },
            max_width: Some(10.0),
            max_height: Some(20.0),
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(4.0, 5.0)).unwrap(),
            Size::new(10.0, 20.0)
        );
    }

    #[test]
    fn scale_with_max_width_and_height_doesnt_fit_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 10.0, y: 20.0 },
            max_width: Some(10.0),
            max_height: Some(15.0),
            keep_aspect_ratio: true,
        };

        assert_eq!(
            // this will end up with a 40:120 aspect ratio
            strategy.apply(&Size::new(4.0, 6.0)).unwrap(),
            // which should be shrunk to 1:3 that fits in (10, 15) per the max_width/max_height above
            Size::new(5.0, 15.0)
        );
    }

    #[test]
    fn scale_with_max_width_fits_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 5.0, y: 20.0 },
            max_width: Some(10.0),
            max_height: None,
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(1.0, 10.0)).unwrap(),
            Size::new(5.0, 200.0),
        );
    }

    #[test]
    fn scale_with_max_width_fits_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 5.0, y: 20.0 },
            max_width: Some(10.0),
            max_height: None,
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(1.0, 10.0)).unwrap(),
            Size::new(5.0, 200.0),
        );
    }

    #[test]
    fn scale_with_max_height_fits_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 20.0, y: 5.0 },
            max_width: None,
            max_height: Some(10.0),
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(10.0, 1.0)).unwrap(),
            Size::new(200.0, 5.0),
        );
    }

    #[test]
    fn scale_with_max_height_fits_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 20.0, y: 5.0 },
            max_width: None,
            max_height: Some(10.0),
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(10.0, 1.0)).unwrap(),
            Size::new(200.0, 5.0),
        );
    }

    #[test]
    fn scale_with_max_width_doesnt_fit_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 10.0, y: 20.0 },
            max_width: Some(10.0),
            max_height: None,
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(5.0, 10.0)).unwrap(),
            Size::new(10.0, 200.0),
        );
    }

    #[test]
    fn scale_with_max_width_doesnt_fit_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 10.0, y: 20.0 },
            max_width: Some(10.0),
            max_height: None,
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(5.0, 10.0)).unwrap(),
            Size::new(10.0, 40.0),
        );
    }

    #[test]
    fn scale_with_max_height_doesnt_fit_non_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 10.0, y: 20.0 },
            max_width: None,
            max_height: Some(10.0),
            keep_aspect_ratio: false,
        };

        assert_eq!(
            strategy.apply(&Size::new(5.0, 10.0)).unwrap(),
            Size::new(50.0, 10.0),
        );
    }

    #[test]
    fn scale_with_max_height_doesnt_fit_proportional() {
        let strategy = ResizeStrategy::ScaleWithMaxSize {
            scale: Scale { x: 8.0, y: 20.0 },
            max_width: None,
            max_height: Some(10.0),
            keep_aspect_ratio: true,
        };

        assert_eq!(
            strategy.apply(&Size::new(5.0, 10.0)).unwrap(),
            Size::new(2.0, 10.0),
        );
    }
}
