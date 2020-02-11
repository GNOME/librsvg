extern crate assert_cmd;
extern crate predicates;

use crate::cmdline::predicates::file;

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

// What should be tested here?
// The goal is to test the code in rsvg-convert, not the entire library.
//
//  - all command-line options are accepted
//  - size of the output (should be sufficient to do that for PNG) ✔
//  - command-line options that affect size (width, height, zoom, resolution) ✔
//  - limit on output size (32767 pixels) ✔
//  - output formats (PNG, PDF, PS, EPS, SVG), okay to ignore XML and recording ✔
//  - multi-page output (for PDF) ✔
//  - handling of SOURCE_DATA_EPOCH environment variable for PDF output
//  - handling of background color option
//  - support for optional CSS stylesheet
//  - error handling for missing SVG dimensions ✔
//  - error handling for export lookup ID
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
fn output_format_unknown_yields_error() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--format=foo")
        .assert()
        .failure()
        .stderr("Unknown output format.\n");
}

#[test]
fn empty_input_yields_error() {
    let starts_with = predicate::str::starts_with("Error reading SVG");
    let ends_with = predicate::str::ends_with("Input file is too short");
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
fn multiple_input_files_accepted_for_pdf_output() {
    let one = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg(one)
        .arg(two)
        .assert()
        .success()
        .stdout(file::is_pdf());
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
        .stderr(predicate::str::ends_with("Could not render file stdin").trim());
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
    let starts_with =
        predicate::str::starts_with("The resulting image would be larger than 32767 pixels");
    let ends_with = predicate::str::ends_with("Please specify a smaller size.");
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
fn version_option() {
    let out = predicate::str::starts_with("rsvg-convert version ");
    RsvgConvert::new()
        .arg("-v")
        .assert()
        .success()
        .stdout(out.clone());
    RsvgConvert::new()
        .arg("--version")
        .assert()
        .success()
        .stdout(out);
}

#[test]
fn help_option() {
    let out = predicate::str::starts_with("Usage:");
    RsvgConvert::new()
        .arg("-?")
        .assert()
        .success()
        .stdout(out.clone());
    RsvgConvert::new()
        .arg("--help")
        .assert()
        .success()
        .stdout(out);
}
