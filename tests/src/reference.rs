//! Tests with reference images.
//!
//! This is the bulk of librsvg's black-box tests.  In principle, each test takes an SVG file, renders
//! it to a raster image, and compares that image to a reference image stored on disk.  If the images
//! are "too different", the test fails.  We allow for minor differences in rendering to account for
//! antialiasing artifacts, floating-point variations, and such.
//!

#![cfg(test)]
use test_generator::test_resources;

use cairo;
use librsvg::{CairoRenderer, IntrinsicDimensions, Length, LengthUnit, Loader};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use crate::utils::fixture_path;

// The original reference images from the SVG1.1 test suite are at 72 DPI.
const TEST_SUITE_DPI: f64 = 72.0;

fn reference_test(name: &str) {
    let path = fixture_path(name);
    if path.file_stem().unwrap().to_string_lossy().starts_with("ignore") {
        return;
    }

    let reference = reference_path(&path);

    let handle = Loader::new()
        .read_path(path)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let renderer = CairoRenderer::new(&handle).with_dpi(TEST_SUITE_DPI, TEST_SUITE_DPI);

    let (width, height) = image_size(renderer.intrinsic_dimensions(), TEST_SUITE_DPI);

    let mut reference_file = BufReader::new(File::open(reference).unwrap());
    let expected = cairo::ImageSurface::create_from_png(&mut reference_file).unwrap();

    assert!(width == expected.get_width() && height == expected.get_height());
}

/// Turns `/foo/bar/baz.svg` into `/foo/bar/baz-ref.svg`.
fn reference_path(path: &PathBuf) -> PathBuf {
    let basename = path.file_stem().unwrap();

    let mut reference_filename = basename.to_string_lossy().into_owned();
    reference_filename.push_str("-ref.png");

    path.with_file_name(reference_filename)
}

/// Computes the (width, height) pixel size at which an SVG should be rendered, based on its intrinsic dimensions.
///
/// # Panics:
///
/// Will panic if none of the following conditions are met:
///
/// * Width and height both exist
/// * Width and height do not exist, but viewBox exists.
fn image_size(dim: IntrinsicDimensions, dpi: f64) -> (i32, i32) {
    let IntrinsicDimensions {
        width,
        height,
        vbox,
    } = dim;

    use LengthUnit::*;

    if let (Some(width), Some(height)) = (width, height) {
        if !(has_supported_unit(&width) && has_supported_unit(&height)) {
            panic!("SVG has unsupported unit type in width or height");
        }
    }

    #[rustfmt::skip]
    let (width, height) = match (width, height, vbox) {
        (Some(Length { length: w, unit: Percent }),
         Some(Length { length: h, unit: Percent }), vbox) if w == 1.0 && h == 1.0 => {
            if let Some(vbox) = vbox {
                (vbox.width, vbox.height)
            } else {
                panic!("SVG with percentage width/height must have a viewBox");
            }
        }

        (Some(Length { length: _, unit: Percent }),
         Some(Length { length: _, unit: Percent }), _) => {
            panic!("Test suite only supports percentage width/height at 100%");
        }

        (Some(w), Some(h), _) => {
            (normalize(&w, dpi), normalize(&h, dpi))
        }

        (None, None, Some(vbox)) => (vbox.width, vbox.height),

        (_, _, _) => panic!("Test suite does not support the dimensions of this file"),
    };

    // Keep in sync with c_api.rs
    let width = checked_i32(width.round());
    let height = checked_i32(height.round());

    (width, height)
}

// Keep in sync with c_api.rs
fn checked_i32(x: f64) -> i32 {
    cast::i32(x).expect("overflow when converting f64 to i32")
}

fn has_supported_unit(l: &Length) -> bool {
    use LengthUnit::*;

    match l.unit {
        Percent | Px | In | Cm | Mm | Pt | Pc => true,
        _ => false,
    }
}

const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH: f64 = 2.54;
const MM_PER_INCH: f64 = 25.4;
const PICA_PER_INCH: f64 = 6.0;

fn normalize(l: &Length, dpi: f64) -> f64 {
    use LengthUnit::*;

    match l.unit {
        Px => l.length,
        In => l.length * dpi,
        Cm => l.length * dpi / CM_PER_INCH,
        Mm => l.length * dpi / MM_PER_INCH,
        Pt => l.length * dpi / POINTS_PER_INCH,
        Pc => l.length * dpi / PICA_PER_INCH,
        _ => panic!("unsupported length unit"),
    }
}

#[test_resources("tests/fixtures/reftests/*.svg")]
fn reftests(name: &str) {
    reference_test(name);
}

#[test_resources("tests/fixtures/reftests/adwaita/*.svg")]
fn adwaita(name: &str) {
    reference_test(name);
}

#[test_resources("tests/fixtures/reftests/bugs/*.svg")]
fn bugs(name: &str) {
    reference_test(name);
}

#[test_resources("tests/fixtures/reftests/svg1.1/*.svg")]
fn svg_1_1(name: &str) {
    reference_test(name);
}

#[test_resources("tests/fixtures/reftests/svg2/*.svg")]
fn svg_2(name: &str) {
    reference_test(name);
}
