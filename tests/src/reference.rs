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
use librsvg::{
    surface_utils::shared_surface::{SharedImageSurface, SurfaceType},
    CairoRenderer, IntrinsicDimensions, Length, Loader,
};
use std::path::PathBuf;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::utils::{setup_font_map, setup_language};

// The original reference images from the SVG1.1 test suite are at 72 DPI.
const TEST_SUITE_DPI: f64 = 72.0;

// https://gitlab.gnome.org/GNOME/librsvg/issues/91
//
// We were computing some offsets incorrectly if the initial transformation matrix
// passed to rsvg_handle_render_cairo() was not the identity matrix.  So,
// we create a surface with a "frame" around the destination for the image,
// and then only consider the pixels inside the frame.  This will require us
// to have a non-identity transformation (i.e. a translation matrix), which
// will test for this bug.
//
// The frame size is meant to be a ridiculous number to simulate an arbitrary
// offset.
const FRAME_SIZE: i32 = 47;

fn reference_test(path: &str) {
    setup_language();
    setup_font_map();

    let path = PathBuf::from(path);
    let path_base_name = path.file_stem().unwrap().to_string_lossy().to_owned();
    if path_base_name.starts_with("ignore") {
        return;
    }

    let reference = reference_path(&path);

    let handle = Loader::new()
        .read_path(&path)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let renderer = CairoRenderer::new(&handle)
        .test_mode()
        .with_dpi(TEST_SUITE_DPI, TEST_SUITE_DPI);
    let (width, height) = image_size(renderer.intrinsic_dimensions(), TEST_SUITE_DPI);

    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        width + 2 * FRAME_SIZE,
        height + 2 * FRAME_SIZE,
    )
    .unwrap();

    {
        let cr = cairo::Context::new(&surface);
        cr.translate(f64::from(FRAME_SIZE), f64::from(FRAME_SIZE));
        renderer
            .render_document(
                &cr,
                &cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: f64::from(width),
                    height: f64::from(height),
                },
            )
            .unwrap();
    }

    let surface = extract_rectangle(&surface, FRAME_SIZE, FRAME_SIZE, width, height).unwrap();

    let output_surf = SharedImageSurface::wrap(surface, SurfaceType::SRgb).unwrap();

    Reference::from_png(&reference)
        .compare(&output_surf)
        .evaluate(&output_surf, &path_base_name);
}

/// Turns `/foo/bar/baz.svg` into `/foo/bar/baz-ref.svg`.
fn reference_path(path: &PathBuf) -> PathBuf {
    let basename = path.file_stem().unwrap();

    let mut reference_filename = basename.to_string_lossy().into_owned();
    reference_filename.push_str("-ref.png");

    path.with_file_name(reference_filename)
}

fn extract_rectangle(
    source: &cairo::ImageSurface,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let dest = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
    let cr = cairo::Context::new(&dest);
    cr.set_source_surface(&source, f64::from(-x), f64::from(-y));
    cr.paint();
    Ok(dest)
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

    use librsvg::LengthUnit::*;

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
    use librsvg::LengthUnit::*;

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
    use librsvg::LengthUnit::*;

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
