extern crate assert_cmd;
extern crate predicates;

use crate::cmdline::png_predicate;

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

// What should be tested here?
// The goal is to test the code in rsvg-convert, not the entire library.
//
//  - all command-line options are accepted
//  - size and resolution of the output (should be sufficient to do that for PNG)
//  - limit on output size (32767 pixels)
//  - output formats (PNG, PDF, PS, EPS, SVG), okay to ignore XML and recording
//  - multi-page output (for PDF)
//  - handling of SOURCE_DATA_EPOCH environment variable for PDF output
//  - handling of background color option
//  - support for optional CSS stylesheet
//  - error handling for export lookup ID
//  - error handling for invalid input

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
        Command::new(path)
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
fn empty_input_yields_error() {
    let starts_with = predicate::str::starts_with("Error reading SVG");
    let ends_with = predicate::str::ends_with("Input file is too short");
    RsvgConvert::new()
        .assert()
        .failure()
        .stderr(starts_with.and(ends_with).trim());
}

#[test]
fn reads_from_stdin() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .assert()
        .success()
        .stdout(png_predicate::has_size(200, 100));
}

#[test]
fn width_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=300")
        .assert()
        .success()
        .stdout(png_predicate::has_size(300, 150));
}

#[test]
fn height_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--height=200")
        .assert()
        .success()
        .stdout(png_predicate::has_size(400, 200));
}

#[test]
fn width_and_height_options() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--width=300")
        .arg("--height=200")
        .assert()
        .success()
        .stdout(png_predicate::has_size(300, 200));
}

#[test]
fn zoom_factor() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--zoom=0.8")
        .assert()
        .success()
        .stdout(png_predicate::has_size(160, 80));
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
        .stdout(png_predicate::has_size(300, 150));
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
        .stdout(png_predicate::has_size(400, 200));
}

#[test]
fn x_zoom_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--x-zoom=2")
        .assert()
        .success()
        .stdout(png_predicate::has_size(400, 100));
}

#[test]
fn x_short_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("-x")
        .arg("2.0")
        .assert()
        .success()
        .stdout(png_predicate::has_size(400, 100));
}

#[test]
fn y_zoom_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("--y-zoom=2.0")
        .assert()
        .success()
        .stdout(png_predicate::has_size(200, 200));
}

#[test]
fn y_short_option() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .arg("-y")
        .arg("2")
        .assert()
        .success()
        .stdout(png_predicate::has_size(200, 200));
}

#[test]
fn huge_zoom_yields_error() {
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
