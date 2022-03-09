use crate::predicates::ends_with_pkg_version;
use crate::predicates::file;

use assert_cmd::assert::IntoOutputPredicate;
use assert_cmd::Command;
#[cfg(system_deps_have_cairo_pdf)]
use chrono::{TimeZone, Utc};
use librsvg::{Length, LengthUnit};
use predicates::boolean::*;
use predicates::prelude::*;
use predicates::str::*;
use std::path::Path;
use tempfile::Builder;
use url::Url;

// What should be tested here?
// The goal is to test the code in rsvg-convert, not the entire library.
//
//  - command-line options that affect size (width, height, zoom, resolution) ✔
//  - pixel dimensions of the output (should be sufficient to do that for PNG) ✔
//  - limit on output size (32767 pixels) ✔
//  - output formats (PNG, PDF, PS, EPS, SVG) ✔
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
    fn new() -> Command {
        Command::cargo_bin("rsvg-convert").unwrap()
    }

    fn new_with_input<P>(file: P) -> Command
    where
        P: AsRef<Path>,
    {
        let mut command = RsvgConvert::new();
        match command.pipe_stdin(&file) {
            Ok(_) => command,
            Err(e) => panic!("Error opening file '{}': {}", file.as_ref().display(), e),
        }
    }

    fn accepts_arg(option: &str) {
        RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
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
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .assert()
        .success()
        .stdout(file::is_png());
}

#[test]
fn argument_is_input_filename() {
    let input = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .arg(input)
        .assert()
        .success()
        .stdout(file::is_png());
}

#[test]
fn argument_is_url() {
    let path = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg")
        .canonicalize()
        .unwrap();
    let url = Url::from_file_path(path).unwrap();
    let stringified = url.as_str();
    assert!(stringified.starts_with("file://"));

    RsvgConvert::new()
        .arg(stringified)
        .assert()
        .success()
        .stdout(file::is_png());
}

#[test]
fn output_format_png() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format=png")
        .assert()
        .success()
        .stdout(file::is_png());
}

#[cfg(system_deps_have_cairo_ps)]
#[test]
fn output_format_ps() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format=ps")
        .assert()
        .success()
        .stdout(file::is_ps());
}

#[cfg(system_deps_have_cairo_ps)]
#[test]
fn output_format_eps() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format=eps")
        .assert()
        .success()
        .stdout(file::is_eps());
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn output_format_pdf() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format=pdf")
        .assert()
        .success()
        .stdout(file::is_pdf());
}

#[cfg(system_deps_have_cairo_svg)]
#[test]
fn output_format_svg_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("-f")
        .arg("svg")
        .assert()
        .success()
        .stdout(file::is_svg());
}

#[cfg(system_deps_have_cairo_svg)]
#[test]
fn user_specified_width_and_height() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format")
        .arg("svg")
        .arg("--width")
        .arg("42cm")
        .arg("--height")
        .arg("43cm")
        .assert()
        .success()
        .stdout(file::is_svg().with_size(
            Length::new(42.0, LengthUnit::Cm),
            Length::new(43.0, LengthUnit::Cm),
        ));
}

#[cfg(system_deps_have_cairo_svg)]
#[test]
fn user_specified_width_and_height_px_output() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format")
        .arg("svg")
        .arg("--width")
        .arg("1920")
        .arg("--height")
        .arg("508mm")
        .assert()
        .success()
        .stdout(file::is_svg().with_size(
            Length::new(1920.0, LengthUnit::Px),
            Length::new(1920.0, LengthUnit::Px),
        ));
}

#[cfg(system_deps_have_cairo_svg)]
#[test]
fn user_specified_width_and_height_a4() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--format")
        .arg("svg")
        .arg("--page-width")
        .arg("210mm")
        .arg("--page-height")
        .arg("297mm")
        .arg("--left")
        .arg("1cm")
        .arg("--top")
        .arg("1cm")
        .arg("--width")
        .arg("190mm")
        .arg("--height")
        .arg("277mm")
        .assert()
        .success()
        .stdout(file::is_svg().with_size(
            Length::new(210.0, LengthUnit::Mm),
            Length::new(297.0, LengthUnit::Mm),
        ));
}

#[test]
fn output_file_option() {
    let output = {
        let tempfile = Builder::new().suffix(".png").tempfile().unwrap();
        tempfile.path().to_path_buf()
    };
    assert!(predicates::path::is_file().not().eval(&output));

    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
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

    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("-o")
        .arg(format!("{}", output.display()))
        .assert()
        .success()
        .stdout(is_empty());

    assert!(predicates::path::is_file().eval(&output));
    std::fs::remove_file(&output).unwrap();
}

#[test]
fn overwrites_existing_output_file() {
    let output = {
        let tempfile = Builder::new().suffix(".png").tempfile().unwrap();
        tempfile.path().to_path_buf()
    };
    assert!(predicates::path::is_file().not().eval(&output));

    for _ in 0..2 {
        RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
            .arg(format!("--output={}", output.display()))
            .assert()
            .success()
            .stdout(is_empty());

        assert!(predicates::path::is_file().eval(&output));
    }

    std::fs::remove_file(&output).unwrap();
}

#[test]
fn empty_input_yields_error() {
    let starts_with = starts_with("Error reading SVG");
    let ends_with = ends_with("Input file is too short").trim();
    RsvgConvert::new()
        .assert()
        .failure()
        .stderr(starts_with.and(ends_with));
}

#[test]
fn empty_svg_yields_error() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/empty.svg")
        .assert()
        .failure()
        .stderr("The SVG stdin has no dimensions\n");
}

#[test]
fn multiple_input_files_not_allowed_for_png_output() {
    let one = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("tests/fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg(one)
        .arg(two)
        .assert()
        .failure()
        .stderr(contains(
            "Multiple SVG files are only allowed for PDF and (E)PS output",
        ));
}

#[cfg(system_deps_have_cairo_ps)]
#[test]
fn multiple_input_files_accepted_for_eps_output() {
    let one = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("tests/fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg("--format=eps")
        .arg(one)
        .arg(two)
        .assert()
        .success()
        .stdout(file::is_eps());
}

#[cfg(system_deps_have_cairo_ps)]
#[test]
fn multiple_input_files_accepted_for_ps_output() {
    let one = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("tests/fixtures/dimensions/sub-rect-no-unit.svg");
    RsvgConvert::new()
        .arg("--format=ps")
        .arg(one)
        .arg(two)
        .assert()
        .success()
        .stdout(file::is_ps());
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn multiple_input_files_create_multi_page_pdf_output() {
    let one = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("tests/fixtures/dimensions/sub-rect-no-unit.svg");
    let three = Path::new("tests/fixtures/api/example.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg(one)
        .arg(two)
        .arg(three)
        .assert()
        .success()
        .stdout(
            file::is_pdf()
                .with_page_count(3)
                .and(file::is_pdf().with_page_size(0, 150.0, 75.0))
                .and(file::is_pdf().with_page_size(1, 123.0, 123.0))
                .and(file::is_pdf().with_page_size(2, 75.0, 300.0)),
        );
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn multiple_input_files_create_multi_page_pdf_output_fixed_size() {
    let one = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("tests/fixtures/dimensions/sub-rect-no-unit.svg");
    let three = Path::new("tests/fixtures/api/example.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg("--page-width=8.5in")
        .arg("--page-height=11in")
        .arg("--width=7.5in")
        .arg("--height=10in")
        .arg("--left=0.5in")
        .arg("--top=0.5in")
        .arg("--keep-aspect-ratio")
        .arg(one)
        .arg(two)
        .arg(three)
        .assert()
        .success()
        .stdout(
            file::is_pdf()
                .with_page_count(3)
                // https://www.wolframalpha.com/input/?i=convert+11+inches+to+desktop+publishing+points
                .and(file::is_pdf().with_page_size(0, 612.0, 792.0))
                .and(file::is_pdf().with_page_size(1, 612.0, 792.0))
                .and(file::is_pdf().with_page_size(2, 612.0, 792.0)),
        );
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn pdf_has_link() {
    let input = Path::new("tests/fixtures/cmdline/a-link.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .success()
        .stdout(file::is_pdf().with_link("https://example.com"));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn pdf_has_link_inside_text() {
    let input = Path::new("tests/fixtures/cmdline/text-a-link.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .success()
        .stdout(
            file::is_pdf()
                .with_link("https://example.com")
                .and(file::is_pdf().with_link("https://another.example.com")),
        );
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn pdf_has_text() {
    let input = Path::new("tests/fixtures/text/hello-world.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .success()
        .stdout(
            file::is_pdf()
                .with_text("Hello world!")
                .and(file::is_pdf().with_text("Hello again!")),
        );
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn env_source_data_epoch_controls_pdf_creation_date() {
    let input = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let date = 1581411039; // seconds since epoch
    RsvgConvert::new()
        .env("SOURCE_DATE_EPOCH", format!("{}", date))
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .success()
        .stdout(file::is_pdf().with_creation_date(Utc.timestamp(date, 0)));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn env_source_data_epoch_no_digits() {
    // intentionally not testing for the full error string here
    let input = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .env("SOURCE_DATE_EPOCH", "foobar")
        .arg("--format=pdf")
        .arg(input)
        .assert()
        .failure()
        .stderr(starts_with("Environment variable $SOURCE_DATE_EPOCH"));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn env_source_data_epoch_trailing_garbage() {
    // intentionally not testing for the full error string here
    let input = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .env("SOURCE_DATE_EPOCH", "1234556+")
        .arg(input)
        .assert()
        .failure()
        .stderr(starts_with("Environment variable $SOURCE_DATE_EPOCH"));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn env_source_data_epoch_empty() {
    // intentionally not testing for the full error string here
    let input = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
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
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--width=300")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 150));
}

#[test]
fn height_option() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--height=200")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 200));
}

#[test]
fn width_and_height_options() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--width=300")
        .arg("--height=200")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 200));
}

#[test]
fn unsupported_unit_in_width_and_height() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--height=200ex")
        .assert()
        .failure()
        .stderr(contains("supported units"));
}

#[test]
fn invalid_length() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--page-width=foo")
        .assert()
        .failure()
        .stderr(contains("can not be parsed as a length"));
}

#[test]
fn zoom_factor() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--zoom=0.8")
        .assert()
        .success()
        .stdout(file::is_png().with_size(160, 80));
}

#[test]
fn zoom_factor_and_larger_size() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--width=400")
        .arg("--height=200")
        .arg("--zoom=1.5")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 150));
}

#[test]
fn zoom_factor_and_smaller_size() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--width=400")
        .arg("--height=200")
        .arg("--zoom=3.5")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 200));
}

#[test]
fn x_zoom_option() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--x-zoom=2")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 100));
}

#[test]
fn x_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("-x")
        .arg("2.0")
        .assert()
        .success()
        .stdout(file::is_png().with_size(400, 100));
}

#[test]
fn y_zoom_option() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--y-zoom=2.0")
        .assert()
        .success()
        .stdout(file::is_png().with_size(200, 200));
}

#[test]
fn y_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("-y")
        .arg("2")
        .assert()
        .success()
        .stdout(file::is_png().with_size(200, 200));
}

#[test]
fn huge_zoom_factor_yields_error() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--zoom=1000")
        .assert()
        .failure()
        .stderr(starts_with(
            "The resulting image would be larger than 32767 pixels",
        ));
}

#[test]
fn negative_zoom_factor_yields_error() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--zoom=-2")
        .assert()
        .failure()
        .stderr(contains("Invalid zoom"));
}

#[test]
fn invalid_zoom_factor_yields_error() {
    RsvgConvert::new_with_input("tests/fixtures/dimensions/521-with-viewbox.svg")
        .arg("--zoom=foo")
        .assert()
        .failure()
        .stderr(contains("Invalid value"));
}

#[test]
fn default_resolution_is_96dpi() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .assert()
        .success()
        .stdout(file::is_png().with_size(96, 384));
}

#[test]
fn x_resolution() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--dpi-x=300")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 384));
}

#[test]
fn x_resolution_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("-d")
        .arg("45")
        .assert()
        .success()
        .stdout(file::is_png().with_size(45, 384));
}

#[test]
fn y_resolution() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--dpi-y=300")
        .assert()
        .success()
        .stdout(file::is_png().with_size(96, 1200));
}

#[test]
fn y_resolution_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("-p")
        .arg("45")
        .assert()
        .success()
        .stdout(file::is_png().with_size(96, 180));
}

#[test]
fn x_and_y_resolution() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--dpi-x=300")
        .arg("--dpi-y=150")
        .assert()
        .success()
        .stdout(file::is_png().with_size(300, 600));
}

#[test]
fn zero_resolution_is_invalid() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--dpi-x=0")
        .arg("--dpi-y=0")
        .assert()
        .failure()
        .stderr(contains("Invalid resolution"));
}

#[test]
fn negative_resolution_is_invalid() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--dpi-x=-100")
        .arg("--dpi-y=-100")
        .assert()
        .failure()
        .stderr(contains("Invalid resolution"));
}

#[test]
fn zero_offset_png() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--page-width=640")
        .arg("--page-height=480")
        .arg("--width=200")
        .arg("--height=100")
        .assert()
        .success()
        .stdout(file::is_png().with_contents("tests/fixtures/cmdline/zero-offset-png.png"));
}

#[test]
fn offset_png() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--page-width=640")
        .arg("--page-height=480")
        .arg("--width=200")
        .arg("--height=100")
        .arg("--left=100")
        .arg("--top=50")
        .assert()
        .success()
        .stdout(file::is_png().with_contents("tests/fixtures/cmdline/offset-png.png"));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn unscaled_pdf_size() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--format=pdf")
        .assert()
        .success()
        .stdout(file::is_pdf().with_page_size(0, 72.0, 72.0));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn pdf_size_width_height() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--format=pdf")
        .arg("--width=2in")
        .arg("--height=3in")
        .assert()
        .success()
        .stdout(file::is_pdf().with_page_size(0, 144.0, 216.0));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn pdf_size_width_height_proportional() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--format=pdf")
        .arg("--width=2in")
        .arg("--height=3in")
        .arg("--keep-aspect-ratio")
        .assert()
        .success()
        .stdout(file::is_pdf().with_page_size(0, 144.0, 144.0));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn pdf_page_size() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--format=pdf")
        .arg("--page-width=210mm")
        .arg("--page-height=297mm")
        .assert()
        .success()
        .stdout(file::is_pdf().with_page_size(0, 210.0 / 25.4 * 72.0, 297.0 / 25.4 * 72.0));
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn multiple_input_files_create_multi_page_pdf_size_override() {
    let one = Path::new("tests/fixtures/dimensions/521-with-viewbox.svg");
    let two = Path::new("tests/fixtures/dimensions/sub-rect-no-unit.svg");
    let three = Path::new("tests/fixtures/api/example.svg");
    RsvgConvert::new()
        .arg("--format=pdf")
        .arg("--width=300pt")
        .arg("--height=200pt")
        .arg(one)
        .arg(two)
        .arg(three)
        .assert()
        .success()
        .stdout(
            file::is_pdf()
                .with_page_count(3)
                .and(file::is_pdf().with_page_size(0, 300.0, 200.0))
                .and(file::is_pdf().with_page_size(1, 300.0, 200.0))
                .and(file::is_pdf().with_page_size(2, 300.0, 200.0)),
        );
}

#[cfg(system_deps_have_cairo_pdf)]
#[test]
fn missing_page_size_yields_error() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--format=pdf")
        .arg("--page-width=210mm")
        .assert()
        .failure()
        .stderr(contains("both").and(contains("options")));

    RsvgConvert::new_with_input("tests/fixtures/cmdline/dimensions-in.svg")
        .arg("--format=pdf")
        .arg("--page-height=297mm")
        .assert()
        .failure()
        .stderr(contains("both").and(contains("options")));
}

#[test]
fn does_not_clip_partial_coverage_pixels() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/677-partial-pixel.svg")
        .assert()
        .success()
        .stdout(file::is_png().with_size(2, 2));
}

#[test]
fn background_color_option_with_valid_color() {
    RsvgConvert::accepts_arg("--background-color=LimeGreen");
}

#[test]
fn background_color_option_none() {
    RsvgConvert::accepts_arg("--background-color=None");
}

#[test]
fn background_color_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("-b")
        .arg("#aabbcc")
        .assert()
        .success();
}

#[test]
fn background_color_option_invalid_color_yields_error() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--background-color=foobar")
        .assert()
        .failure()
        .stderr(contains("Invalid").and(contains("color")));
}

#[test]
fn background_color_is_rendered() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/gimp-wilber.svg")
        .arg("--background-color=purple")
        .assert()
        .success()
        .stdout(file::is_png().with_contents("tests/fixtures/cmdline/gimp-wilber-ref.png"));
}

#[test]
fn stylesheet_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--stylesheet=tests/fixtures/dimensions/empty.svg")
        .assert()
        .success();
}

#[test]
fn stylesheet_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("-s")
        .arg("tests/fixtures/dimensions/empty.svg")
        .assert()
        .success();
}

#[test]
fn stylesheet_option_error() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--stylesheet=foobar")
        .assert()
        .failure()
        .stderr(starts_with("Error reading stylesheet"));
}

#[test]
fn export_id_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/geometry-element.svg")
        .arg("--export-id=foo")
        .assert()
        .success()
        .stdout(file::is_png().with_size(40, 50));
}

#[test]
fn export_id_with_zero_stroke_width() {
    // https://gitlab.gnome.org/GNOME/librsvg/-/issues/601
    //
    // This tests a bug that manifested itself easily with the --export-id option, but it
    // is not a bug with the option itself.  An object with stroke_width=0 was causing
    // an extra point at the origin to be put in the bounding box, so the final image
    // spanned the origin to the actual visible bounds of the rendered object.
    //
    // We can probably test this more cleanly once we have a render tree.
    RsvgConvert::new_with_input("tests/fixtures/cmdline/601-zero-stroke-width.svg")
        .arg("--export-id=foo")
        .assert()
        .success()
        .stdout(
            file::is_png()
                .with_contents("tests/fixtures/cmdline/601-zero-stroke-width-render-only-foo.png"),
        );
}

#[test]
fn export_id_short_option() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("-i")
        .arg("two")
        .assert()
        .success()
        .stdout(file::is_png().with_size(100, 200));
}

#[test]
fn export_id_with_hash_prefix() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("-i")
        .arg("#two")
        .assert()
        .success()
        .stdout(file::is_png().with_size(100, 200));
}

#[test]
fn export_id_option_error() {
    RsvgConvert::new_with_input("tests/fixtures/api/dpi.svg")
        .arg("--export-id=foobar")
        .assert()
        .failure()
        .stderr(starts_with("File stdin does not have an object with id \""));
}

#[test]
fn unlimited_option() {
    RsvgConvert::accepts_arg("--unlimited");
}

#[test]
fn unlimited_short_option() {
    RsvgConvert::accepts_arg("-u");
}

#[test]
fn keep_aspect_ratio_option() {
    let input = Path::new("tests/fixtures/api/dpi.svg");
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
        .stdout(file::is_png().with_size(250, 1000));
}

#[test]
fn keep_aspect_ratio_short_option() {
    let input = Path::new("tests/fixtures/api/dpi.svg");
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
    RsvgConvert::new_with_input("tests/fixtures/render-crash/591-vbox-overflow.svg")
        .assert()
        .failure()
        .stderr(starts_with(
            "The resulting image would be larger than 32767 pixels",
        ));
}

#[test]
fn accept_language_given() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/accept-language.svg")
        .arg("--accept-language=es-MX")
        .assert()
        .success()
        .stdout(file::is_png().with_contents("tests/fixtures/cmdline/accept-language-es.png"));

    RsvgConvert::new_with_input("tests/fixtures/cmdline/accept-language.svg")
        .arg("--accept-language=de")
        .assert()
        .success()
        .stdout(file::is_png().with_contents("tests/fixtures/cmdline/accept-language-de.png"));
}

#[test]
fn accept_language_fallback() {
    RsvgConvert::new_with_input("tests/fixtures/cmdline/accept-language.svg")
        .arg("--accept-language=fr")
        .assert()
        .success()
        .stdout(
            file::is_png().with_contents("tests/fixtures/cmdline/accept-language-fallback.png"),
        );
}

#[test]
fn accept_language_invalid_tag() {
    // underscores are not valid in BCP47 language tags
    RsvgConvert::new_with_input("tests/fixtures/cmdline/accept-language.svg")
        .arg("--accept-language=foo_bar")
        .assert()
        .failure()
        .stderr(contains("invalid language tag"));
}

#[test]
fn keep_image_data_option() {
    RsvgConvert::accepts_arg("--keep-image-data");
}

#[test]
fn no_keep_image_data_option() {
    RsvgConvert::accepts_arg("--no-keep-image-data");
}

fn is_version_output() -> AndPredicate<StartsWithPredicate, TrimPredicate<EndsWithPredicate>, str> {
    starts_with("rsvg-convert version ").and(ends_with_pkg_version().trim())
}

#[test]
fn version_option() {
    RsvgConvert::option_yields_output("--version", is_version_output())
}

#[test]
fn version_short_option() {
    RsvgConvert::option_yields_output("-v", is_version_output())
}

fn is_usage_output() -> OrPredicate<ContainsPredicate, ContainsPredicate, str> {
    contains("Usage:").or(contains("USAGE:"))
}

#[test]
fn help_option() {
    RsvgConvert::option_yields_output("--help", is_usage_output())
}

#[test]
fn help_short_option() {
    RsvgConvert::option_yields_output("-?", is_usage_output())
}
