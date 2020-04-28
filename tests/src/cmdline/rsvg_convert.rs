extern crate assert_cmd;
extern crate chrono;
extern crate predicates;
extern crate tempfile;

use crate::predicates::file;

use assert_cmd::assert::IntoOutputPredicate;
use assert_cmd::Command;
use chrono::{TimeZone, Utc};
use predicate::str::*;
use predicates::prelude::*;
use std::path::Path;
use tempfile::Builder;

// What should be tested here?
// The goal is to test the code in rsvg-convert, not the entire library.
//
//  - command-line options that affect size (width, height, zoom, resolution) ✔
//  - pixel dimensions of the output (should be sufficient to do that for PNG) ✔
//  - limit on output size (32767 pixels) ✔
//  - output formats (PNG, PDF, PS, EPS, SVG), okay to ignore XML and recording ✔
//  - multi-page output (for PDF) ✔
//  - output file option ✔
//  - SOURCE_DATA_EPOCH environment variable for PDF output ✔
//  - background color option ✔
//  - optional CSS stylesheet ✔
//  - error handling for missing SVG dimensions ✔
//  - error handling for export lookup ID ✔
//  - error handling for invalid input ✔

struct RsvgConvert {}

impl RsvgConvert {
    fn binary_location() -> &'static Path {
        match option_env!("LIBRSVG_BUILD_DIR") {
            Some(dir) => Path::new(dir),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
        }
    }

    fn new() -> Command {
        let path = Self::binary_location().join("rsvg-convert");
        let mut command = Command::new(path);
        command.env_clear();
        command
    }

    fn new_with_input(input: &Path) -> Command {
        let mut command = RsvgConvert::new();
        match command.pipe_stdin(input) {
            Ok(_) => command,
            Err(e) => panic!("Error opening file '{}': {}", input.display(), e),
        }
    }

    fn accepts_option(option: &str) {
        let input = Path::new("fixtures/api/dpi.svg");
        RsvgConvert::new_with_input(input)
            .arg(option)
            .assert()
            .success();
    }

    fn option_yields_output<I, P>(option: &str, output_pred: I)
    where
        I: IntoOutputPredicate<P>,
        P: Predicate<[u8]>,
    {
        RsvgConvert::new()
            .arg(option)
            .assert()
            .success()
            .stdout(output_pred);
    }
}

#[test]
fn converts_svg_from_stdin_to_png() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .assert()
        .success()
        .stdout(file::is_png());
}

#[test]
fn argument_is_input_filename() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .arg(input)
        .assert()
        .success()
        .stdout(file::is_png());
}

#[test]
fn output_format_png() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--format=png")
        .assert()
        .success()
        .stdout(file::is_png());
}

#[test]
fn output_format_ps() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--format=ps")
        .assert()
        .success()
        .stdout(file::is_ps());
}

#[test]
fn output_format_eps() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--format=eps")
        .assert()
        .success()
        .stdout(file::is_eps());
}

#[test]
fn output_format_pdf() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--format=pdf")
        .assert()
        .success()
        .stdout(file::is_pdf());
}

#[test]
fn output_format_svg_short_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("-f")
        .arg("svg")
        .assert()
        .success()
        .stdout(file::is_svg());
}

#[test]
fn output_file_option() {
    let output = {
        let tempfile = Builder::new().suffix(".png").tempfile().unwrap();
        tempfile.path().to_path_buf()
    };
    assert!(predicates::path::is_file().not().eval(&output));

    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg(format!("--output={}", output.display()))
        .assert()
        .success()
        .stdout(is_empty());

    assert!(predicates::path::is_file().eval(&output));
    std::fs::remove_file(&output).unwrap();
}

#[test]
fn output_file_short_option() {
    let output = {
        let tempfile = Builder::new().suffix(".png").tempfile().unwrap();
        tempfile.path().to_path_buf()
    };
    assert!(predicates::path::is_file().not().eval(&output));

    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("-o")
        .arg(format!("{}", output.display()))
        .assert()
        .success()
        .stdout(is_empty());

    assert!(predicates::path::is_file().eval(&output));
    std::fs::remove_file(&output).unwrap();
}

#[test]
fn empty_input_yields_error() {
    let starts_with = starts_with("Error reading SVG");
    let ends_with = ends_with("Input file is too short");
    RsvgConvert::new()
        .assert()
        .failure()
        .stderr(starts_with.and(ends_with).trim());
}

#[test]
fn empty_svg_yields_error() {
    let input = Path::new("fixtures/dimensions/empty.svg");
    RsvgConvert::new_with_input(input)
        .assert()
        .failure()
        .stderr("The SVG stdin has no dimensions\n");
}

#[test]
fn multiple_input_files_not_allowed_for_png_output() {
    let one = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg(one)
        .arg(two)
        .assert()
        .failure()
        .stderr("Multiple SVG files are only allowed for PDF and (E)PS output.\n");
}

#[test]
fn multiple_input_files_accepted_for_eps_output() {
    let one = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg("--format=eps")
        .arg(one)
        .arg(two)
        .assert()
        .success()
        .stdout(file::is_eps());
}

#[test]
fn multiple_input_files_accepted_for_ps_output() {
    let one = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg("--format=ps")
        .arg(one)
        .arg(two)
        .assert()
        .success()
        .stdout(file::is_ps());
}

#[test]
fn multiple_input_files_create_multi_page_pdf_output() {
    let one = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("fixtures/dimensions/sub-rect-no-unit.svg");
    let three = Path::new("fixtures/api/example.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg(one)
        .arg(two)
        .arg(three)
        .assert()
        .success()
        .stdout(file::is_pdf().with_page_count(3));
}

#[test]
fn env_source_data_epoch_controls_pdf_creation_date() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let date = 1581411039; // seconds since epoch
    RsvgConvert::new()
        .env("SOURCE_DATE_EPOCH", format!("{}", date))
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .success()
        .stdout(file::is_pdf().with_creation_date(Utc.timestamp(date, 0)));
}

#[test]
fn env_source_data_epoch_no_digits() {
    // intentionally not testing for the full error string here
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .env("SOURCE_DATE_EPOCH", "foobar")
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .failure()
        .stderr(starts_with("Environment variable $SOURCE_DATE_EPOCH"));
}

#[test]
fn env_source_data_epoch_trailing_garbage() {
    // intentionally not testing for the full error string here
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .env("SOURCE_DATE_EPOCH", "1234556+")
        .arg(input)
        .assert()
        .failure()
        .stderr(starts_with("Environment variable $SOURCE_DATE_EPOCH"));
}

#[test]
fn env_source_data_epoch_empty() {
    // intentionally not testing for the full error string here
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .env("SOURCE_DATE_EPOCH", "")
        .arg(input)
        .assert()
        .failure()
        .stderr(starts_with("Environment variable $SOURCE_DATE_EPOCH"));
}

#[test]
fn width_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=300")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 150));
}

#[test]
fn height_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--height=200")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 200));
}

#[test]
fn width_and_height_options() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=300")
        .arg("--height=200")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 200));
}

#[test]
fn zoom_factor() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--zoom=0.8")
        .assert()
        .success()
        .stdout(file::is_png().with_size(160, 80));
}

// TODO: Is this a bug in rsvg-convert or the desired behavior ?
#[test]
fn zoom_factor_and_width_conflicts() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=400")
        .arg("--zoom=1.5")
        .assert()
        .failure()
        .stderr(ends_with("Could not render file stdin").trim());
}

#[test]
fn zoom_factor_and_larger_size() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=400")
        .arg("--height=200")
        .arg("--zoom=1.5")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 150));
}

#[test]
fn zoom_factor_and_smaller_size() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=400")
        .arg("--height=200")
        .arg("--zoom=3.5")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 200));
}

#[test]
fn x_zoom_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--x-zoom=2")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 100));
}

#[test]
fn x_short_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("-x")
        .arg("2.0")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 100));
}

#[test]
fn y_zoom_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--y-zoom=2.0")
        .assert()
        .success()
        .stdout(file::is_png().with_size(200, 200));
}

#[test]
fn y_short_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("-y")
        .arg("2")
        .assert()
        .success()
        .stdout(file::is_png().with_size(200, 200));
}

#[test]
fn huge_zoom_factor_yields_error() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let starts_with = starts_with("The resulting image would be larger than 32767 pixels");
    let ends_with = ends_with("Please specify a smaller size.");
    RsvgConvert::new_with_input(input)
        .arg("--zoom=1000")
        .assert()
        .failure()
        .stderr(starts_with.and(ends_with).trim());
}

#[test]
fn default_resolution_is_90dpi() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .assert()
        .success()
        .stdout(file::is_png().with_size(90, 360));
}

#[test]
fn x_resolution() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--dpi-x=300")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 360));
}

#[test]
fn x_resolution_short_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("-d")
        .arg("45")
        .assert()
        .success()
        .stdout(file::is_png().with_size(45, 360));
}

#[test]
fn y_resolution() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--dpi-y=300")
        .assert()
        .success()
        .stdout(file::is_png().with_size(90, 1200));
}

#[test]
fn y_resolution_short_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("-p")
        .arg("45")
        .assert()
        .success()
        .stdout(file::is_png().with_size(90, 180));
}

#[test]
fn x_and_y_resolution() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--dpi-x=300")
        .arg("--dpi-y=150")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 600));
}

#[test]
fn defaults_are_used_for_zero_resolutions() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--dpi-x=0")
        .arg("--dpi-y=0")
        .assert()
        .success()
        .stdout(file::is_png().with_size(90, 360));
}

#[test]
fn defaults_are_used_for_negative_resolutions() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--dpi-x=-100")
        .arg("--dpi-y=-100")
        .assert()
        .success()
        .stdout(file::is_png().with_size(90, 360));
}

#[test]
fn pdf_page_size() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--format=pdf")
        .assert()
        .success()
        // TODO: the PDF size and resolution is actually a bug in rsvg-convert,
        // see https://gitlab.gnome.org/GNOME/librsvg/issues/514
        .stdout(file::is_pdf().with_page_size(200, 100, 72.0));
}

#[test]
fn background_color_option_with_valid_color() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--background-color=purple")
        .assert()
        .success();
}

#[test]
fn background_color_option_none() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--background-color=None")
        .assert()
        .success();
}

#[test]
fn background_color_short_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("-b")
        .arg("#aabbcc")
        .assert()
        .success();
}

#[test]
fn background_color_option_invalid_color_yields_error() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--background-color=foobar")
        .assert()
        .failure()
        .stderr("Invalid color specification.\n");
}

#[test]
fn stylesheet_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--stylesheet=fixtures/dimensions/empty.svg")
        .assert()
        .success();
}

#[test]
fn stylesheet_short_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("-s")
        .arg("fixtures/dimensions/empty.svg")
        .assert()
        .success();
}

#[test]
fn stylesheet_option_error() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--stylesheet=foobar")
        .assert()
        .failure()
        .stderr(starts_with("Error reading stylesheet"));
}

#[test]
fn export_id_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--export-id=one")
        .assert()
        .success();
}

#[test]
fn export_id_short_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("-i")
        .arg("two")
        .assert()
        .success();
}

#[test]
fn export_id_option_error() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--export-id=foobar")
        .assert()
        .failure()
        .stderr("File stdin does not have an object with id \"foobar\"\n");
}

#[test]
fn unlimited_option() {
    RsvgConvert::accepts_option("--unlimited");
}

#[test]
fn unlimited_short_option() {
    RsvgConvert::accepts_option("-u");
}

#[test]
fn keep_aspect_ratio_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=500")
        .arg("--height=1000")
        .assert()
        .success()
        .stdout(file::is_png().with_size(500, 1000));
    RsvgConvert::new_with_input(input)
        .arg("--width=500")
        .arg("--height=1000")
        .arg("--keep-aspect-ratio")
        .assert()
        .success()
        .stdout(file::is_png().with_size(500, 2000));
}

#[test]
fn keep_aspect_ratio_short_option() {
    let input = Path::new("fixtures/api/dpi.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=1000")
        .arg("--height=500")
        .assert()
        .success()
        .stdout(file::is_png().with_size(1000, 500));
    RsvgConvert::new_with_input(input)
        .arg("--width=1000")
        .arg("--height=500")
        .arg("-a")
        .assert()
        .success()
        .stdout(file::is_png().with_size(125, 500));
}

#[test]
fn overflowing_size_is_detected() {
    let input = Path::new("fixtures/render-crash/591-vbox-overflow.svg");
    RsvgConvert::new_with_input(input)
        .assert()
        .failure()
        .stderr(contains("Could not get dimensions").trim());
}

#[test]
fn keep_image_data_option() {
    RsvgConvert::accepts_option("--keep-image-data");
}

#[test]
fn no_keep_image_data_option() {
    RsvgConvert::accepts_option("--no-keep-image-data");
}

#[test]
fn version_option() {
    RsvgConvert::option_yields_output("--version", starts_with("rsvg-convert version "));
}

#[test]
fn version_short_option() {
    RsvgConvert::option_yields_output("-v", starts_with("rsvg-convert version "));
}

#[test]
fn help_option() {
    RsvgConvert::option_yields_output("--help", starts_with("Usage:"));
}

#[test]
fn help_short_option() {
    RsvgConvert::option_yields_output("-?", starts_with("Usage:"));
}
