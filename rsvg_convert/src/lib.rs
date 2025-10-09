//! This crate exists only to enable testing (by the `ci/check-rsvg-convert-options`
//! crate) that rsvg-convert's CLI options are fully and properly documented in its
//! man page (`rsvg-convert.rst`).

use clap::crate_version;
use clap_complete::Shell;

use rsvg::rsvg_convert_only::{
    CssLength, Horizontal, Normalize, Parse, Signed, Unsigned, Validate, Vertical,
};
use rsvg::LengthUnit;

use std::ffi::OsString;
use std::path::PathBuf;

pub fn build_cli() -> clap::Command {
    let supported_formats = vec![
        "png",
        #[cfg(system_deps_have_cairo_pdf)]
        "pdf",
        #[cfg(system_deps_have_cairo_pdf)]
        "pdf1.7",
        #[cfg(system_deps_have_cairo_pdf)]
        "pdf1.6",
        #[cfg(system_deps_have_cairo_pdf)]
        "pdf1.5",
        #[cfg(system_deps_have_cairo_pdf)]
        "pdf1.4",
        #[cfg(system_deps_have_cairo_ps)]
        "ps",
        #[cfg(system_deps_have_cairo_ps)]
        "eps",
        #[cfg(system_deps_have_cairo_svg)]
        "svg",
    ];

    // If any change is made to these options, please update `rsvg-convert.rst`
    // (in the repository root) accordingly, run:
    //
    //   $ cargo run -p ci --bin check-rsvg-convert-options
    //
    // and make the neccesary corrections, if there are errors.

    clap::Command::new("rsvg-convert")
        .version(concat!("version ", crate_version!()))
        .about("Convert SVG files to other image formats")
        .disable_version_flag(true)
        .disable_help_flag(true)
        .arg(
            clap::Arg::new("help")
                .short('?')
                .long("help")
                .help("Display the help")
                .action(clap::ArgAction::Help)
        )
        .arg(
            clap::Arg::new("version")
                .short('v')
                .long("version")
                .help("Display the version information")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("res_x")
                .short('d')
                .long("dpi-x")
                .num_args(1)
                .value_name("number")
                .default_value("96")
                .value_parser(parse_resolution)
                .help("Pixels per inch")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("res_y")
                .short('p')
                .long("dpi-y")
                .num_args(1)
                .value_name("number")
                .default_value("96")
                .value_parser(parse_resolution)
                .help("Pixels per inch")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("zoom_x")
                .short('x')
                .long("x-zoom")
                .num_args(1)
                .value_name("number")
                .conflicts_with("zoom")
                .value_parser(parse_zoom_factor)
                .help("Horizontal zoom factor")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("zoom_y")
                .short('y')
                .long("y-zoom")
                .num_args(1)
                .value_name("number")
                .conflicts_with("zoom")
                .value_parser(parse_zoom_factor)
                .help("Vertical zoom factor")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("zoom")
                .short('z')
                .long("zoom")
                .num_args(1)
                .value_name("number")
                .value_parser(parse_zoom_factor)
                .help("Zoom factor")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("size_x")
                .short('w')
                .long("width")
                .num_args(1)
                .value_name("length")
                .value_parser(parse_length::<Horizontal, Unsigned>)
                .help("Width [defaults to the width of the SVG]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("size_y")
                .short('h')
                .long("height")
                .num_args(1)
                .value_name("length")
                .value_parser(parse_length::<Vertical, Unsigned>)
                .help("Height [defaults to the height of the SVG]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("top")
                .long("top")
                .num_args(1)
                .value_name("length")
                .value_parser(parse_length::<Vertical, Signed>)
                .help("Distance between top edge of page and the image [defaults to 0]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("left")
                .long("left")
                .num_args(1)
                .value_name("length")
                .value_parser(parse_length::<Horizontal, Signed>)
                .help("Distance between left edge of page and the image [defaults to 0]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("page_width")
                .long("page-width")
                .num_args(1)
                .value_name("length")
                .value_parser(parse_length::<Horizontal, Unsigned>)
                .help("Width of output media [defaults to the width of the SVG]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("page_height")
                .long("page-height")
                .num_args(1)
                .value_name("length")
                .value_parser(parse_length::<Vertical, Unsigned>)
                .help("Height of output media [defaults to the height of the SVG]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("format")
                .short('f')
                .long("format")
                .num_args(1)
                .value_parser(clap::builder::PossibleValuesParser::new(supported_formats.as_slice()))
                .ignore_case(true)
                .default_value("png")
                .help("Output format")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("output")
                .short('o')
                .long("output")
                .num_args(1)
                .value_parser(clap::value_parser!(PathBuf))
                .value_name("filename")
                .help("Output filename [defaults to stdout]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("export_id")
                .short('i')
                .long("export-id")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .value_name("object-id")
                .help("SVG id of object to export [default is to export all objects]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("accept-language")
                .short('l')
                .long("accept-language")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .value_name("language-tags")
                .help("Languages to accept, for example \"es-MX,de,en\" [default uses language from the environment]")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("keep_aspect")
                .short('a')
                .long("keep-aspect-ratio")
                .help("Preserve the aspect ratio")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("background")
                .short('b')
                .long("background-color")
                .num_args(1)
                .value_name("color")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .default_value("none")
                .help("Set the background color using a CSS color spec")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("stylesheet")
                .short('s')
                .long("stylesheet")
            .num_args(1)
                .value_parser(clap::value_parser!(PathBuf))
                .value_name("filename.css")
                .help("Filename of CSS stylesheet to apply")
                .action(clap::ArgAction::Set),
        )
        .arg(
            clap::Arg::new("unlimited")
                .short('u')
                .long("unlimited")
                .help("Allow huge SVG files")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("keep_image_data")
                .long("keep-image-data")
                .help("Keep image data")
                .conflicts_with("no_keep_image_data")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("no_keep_image_data")
                .long("no-keep-image-data")
                .help("Do not keep image data")
                .conflicts_with("keep_image_data")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("testing")
                .long("testing")
                .help("Render images for librsvg's test suite")
                .hide(true)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("completion")
                .long("completion")
                .help("Output shell completion for the given shell")
                .num_args(1)
                .action(clap::ArgAction::Set)
                .value_parser(clap::value_parser!(Shell))
                .value_name("shell-name"),
        )
        .arg(
            clap::Arg::new("FILE")
                .value_parser(clap::value_parser!(OsString))
                .help("The input file(s) to convert, you can use - for stdin")
                .num_args(1..)
                .action(clap::ArgAction::Append),
        )
}

#[derive(Copy, Clone)]
pub struct Resolution(pub f64);

fn parse_resolution(v: &str) -> Result<Resolution, String> {
    match v.parse::<f64>() {
        Ok(res) if res > 0.0 => Ok(Resolution(res)),
        Ok(_) => Err(String::from("Invalid resolution")),
        Err(e) => Err(format!("{e}")),
    }
}

#[derive(Copy, Clone)]
pub struct ZoomFactor(pub f64);

fn parse_zoom_factor(v: &str) -> Result<ZoomFactor, String> {
    match v.parse::<f64>() {
        Ok(res) if res > 0.0 => Ok(ZoomFactor(res)),
        Ok(_) => Err(String::from("Invalid zoom factor")),
        Err(e) => Err(format!("{e}")),
    }
}

fn is_absolute_unit(u: LengthUnit) -> bool {
    use LengthUnit::*;

    match u {
        Percent | Em | Ex | Ch => false,
        Px | In | Cm | Mm | Pt | Pc => true,

        // coverage: the following is because LengthUnit is marked non_exhaustive, but
        // the cases above should really test all the units librsvg knows about.
        _ => false,
    }
}

fn parse_length<N: Normalize, V: Validate>(s: &str) -> Result<CssLength<N, V>, String> {
    <CssLength<N, V> as Parse>::parse_str(s)
        .map_err(|_| format!("Invalid value: The argument '{s}' can not be parsed as a length"))
        .and_then(|l| {
            if is_absolute_unit(l.unit) {
                Ok(l)
            } else {
                Err(format!(
                    "Invalid value '{s}': supported units are px, in, cm, mm, pt, pc"
                ))
            }
        })
}
