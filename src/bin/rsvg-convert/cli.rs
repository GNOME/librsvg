// command-line interface for rsvg-convert

use std::path::PathBuf;

use librsvg::{Color, Parse};

use crate::input::Input;
use crate::output::Output;
use crate::size::{Dpi, Scale};

arg_enum! {
    #[derive(Clone, Copy, Debug)]
    pub enum Format {
        Png,
        Pdf,
        Ps,
        Eps,
        Svg,
    }
}

#[derive(Debug)]
pub struct Args {
    pub dpi: Dpi,
    pub zoom: Scale,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Format,
    export_id: Option<String>,
    pub keep_aspect_ratio: bool,
    pub background_color: Option<Color>,
    pub stylesheet: Option<PathBuf>,
    pub unlimited: bool,
    pub keep_image_data: bool,
    pub output: Output,
    pub input: Vec<Input>,
}

impl Args {
    pub fn new() -> Result<Self, clap::Error> {
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

        let lookup_id = |id: String| {
            // RsvgHandle::has_sub() expects ids to have a '#' prepended to them,
            // so it can lookup ids in externs like "subfile.svg#subid".  For the
            // user's convenience, we include this '#' automatically; we only
            // support specifying ids from the toplevel, and don't expect users to
            // lookup things in externs.
            if id.starts_with('#') {
                id
            } else {
                format!("#{}", id)
            }
        };

        let zoom = value_t!(matches, "zoom", f64).or_none()?;
        let zoom_x = value_t!(matches, "zoom_x", f64).or_none()?;
        let zoom_y = value_t!(matches, "zoom_y", f64).or_none()?;

        let args = Args {
            dpi: Dpi {
                x: value_t!(matches, "res_x", f64)?,
                y: value_t!(matches, "res_y", f64)?,
            },
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
            output: matches
                .value_of_os("output")
                .map(PathBuf::from)
                .map(Output::Path)
                .unwrap_or(Output::Stdout),
            input: match matches.values_of_os("FILE") {
                Some(values) => values.map(PathBuf::from).map(Input::Path).collect(),
                None => vec![Input::Stdin],
            },
        };

        if args.input.len() > 1 {
            match args.format {
                Format::Ps | Format::Eps | Format::Pdf => (),
                _ => {
                    return Err(clap::Error::with_description(
                        "Multiple SVG files are only allowed for PDF and (E)PS output.",
                        clap::ErrorKind::TooManyValues,
                    ))
                }
            }
        }

        Ok(args)
    }

    pub fn export_id(&self) -> Option<&str> {
        self.export_id.as_deref()
    }
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

fn parse_color_string(str: String) -> Result<Color, clap::Error> {
    parse_color_str(&str)
}

fn parse_color_str(str: &str) -> Result<Color, clap::Error> {
    match str {
        "none" | "None" => Err(clap::Error::with_description(
            str,
            clap::ErrorKind::ArgumentNotFound,
        )),
        _ => <Color as Parse>::parse_str(str).map_err(|_| {
            let desc = format!(
                "Invalid value: The argument '{}' can not be parsed as a CSS color value",
                str
            );
            clap::Error::with_description(&desc, clap::ErrorKind::InvalidValue)
        }),
    }
}

#[cfg(test)]
mod tests {
    mod color {
        use super::super::*;

        #[test]
        fn valid_color_is_ok() {
            assert!(parse_color_str("Red").is_ok());
        }

        #[test]
        fn none_is_handled_as_not_found() {
            assert_eq!(
                parse_color_str("None").map_err(|e| e.kind),
                Err(clap::ErrorKind::ArgumentNotFound)
            );
        }

        #[test]
        fn invalid_is_handled_as_invalid_value() {
            assert_eq!(
                parse_color_str("foo").map_err(|e| e.kind),
                Err(clap::ErrorKind::InvalidValue)
            );
        }
    }
}
