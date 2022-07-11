//! Tests with reference images.
//!
//! This is the bulk of librsvg's black-box tests.  In principle, each test takes an SVG file, renders
//! it to a raster image, and compares that image to a reference image stored on disk.  If the images
//! are "too different", the test fails.  We allow for minor differences in rendering to account for
//! antialiasing artifacts, floating-point variations, and such.
//!

#![cfg(test)]
use crate::test_compare_render_output;
use test_generator::test_resources;

use cairo;
use librsvg::{
    surface_utils::shared_surface::{SharedImageSurface, SurfaceType},
    CairoRenderer, IntrinsicDimensions, Length, Loader,
};
use std::path::PathBuf;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::test_svg_reference;
use crate::utils::{load_svg, render_document, setup_font_map, setup_language, SurfaceSize};

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
        .test_mode(true)
        .with_dpi(TEST_SUITE_DPI, TEST_SUITE_DPI);
    let (width, height) = image_size(renderer.intrinsic_dimensions(), TEST_SUITE_DPI);

    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        width + 2 * FRAME_SIZE,
        height + 2 * FRAME_SIZE,
    )
    .unwrap();

    {
        let cr = cairo::Context::new(&surface).expect("Failed to create a cairo context");
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

/// Turns `/foo/bar/baz.svg` into `/foo/bar/baz-ref.png`.
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
) -> Result<cairo::ImageSurface, cairo::Error> {
    let dest = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
    let cr = cairo::Context::new(&dest).expect("Failed to create a cairo context");
    cr.set_source_surface(&source, f64::from(-x), f64::from(-y))
        .unwrap();
    cr.paint().unwrap();
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

    if !(has_supported_unit(&width) && has_supported_unit(&height)) {
        panic!("SVG has unsupported unit type in width or height");
    }

    #[rustfmt::skip]
    let (width, height) = match (width, height, vbox) {
        (Length { length: w, unit: Percent },
         Length { length: h, unit: Percent }, vbox) if w == 1.0 && h == 1.0 => {
            if let Some(vbox) = vbox {
                (vbox.width, vbox.height)
            } else {
                panic!("SVG with percentage width/height must have a viewBox");
            }
        }

        (Length { length: _, unit: Percent },
         Length { length: _, unit: Percent }, _) => {
            panic!("Test suite only supports percentage width/height at 100%");
        }

        (w, h, _) => {
            (normalize(&w, dpi), normalize(&h, dpi))
        }
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

test_compare_render_output!(
    marker_orient_auto_start_reverse,
    100,
    100,
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
    <defs>
      <marker id="marker" orient="auto-start-reverse" viewBox="0 0 10 10"
              refX="0" refY="5" markerWidth="10" markerHeight="10"
              markerUnits="userSpaceOnUse">
        <path d="M0,0 L10,5 L0,10 Z" fill="green"/>
      </marker>
    </defs>
  
    <path d="M20,50 L80,50" marker-start="url(#marker)" marker-end="url(#marker)" stroke-width="10" stroke="black"/>
  </svg>"##,

    br##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <path
         d="M 20,55 10,50 20,45 Z"
         id="triangle1" fill="green"/>
      <path
         d="m 80,45 10,5 -10,5 z"
         id="triangle2" fill="green"/>
      <rect
         id="rectangle"
         width="60"
         height="10"
         x="20"
         y="45" fill="black"/>
    </svg>"##,
);

test_compare_render_output!(
    marker_context_stroke_fill,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
      <style>
        .path1 {
          fill: none;
          stroke-width: 4px;
          marker: url(#marker1);
        }
    
        .path2 {
          fill: darkblue;
          stroke: mediumseagreen;
          stroke-width: 4px;
          marker: url(#marker2);
        }
      </style>
    
      <path class="path1" d="M20,20 L200,20 L380,20" stroke="lime"/>
    
      <path class="path2" d="M20,40 h360 v320 h-360 v-320 Z"/>
    
      <marker id="marker1" markerWidth="12" markerHeight="12" refX="6" refY="6"
              markerUnits="userSpaceOnUse">
        <circle cx="6" cy="6" r="3"
                fill="white" stroke="context-stroke" stroke-width="2"/>
      </marker>
    
      <marker id="marker2" markerWidth="12" markerHeight="12" refX="6" refY="6"
              markerUnits="userSpaceOnUse">
        <!-- Note that here the paint is reversed:
             fill=context-stroke,
             stroke=context-fill 
        -->
        <circle cx="6" cy="6" r="3"
                fill="context-stroke" stroke="context-fill" stroke-width="2"/>
      </marker>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
      <path d="M20,20 L200,20 L380,20" stroke="lime" stroke-width="4"/>
      <circle cx="20" cy="20" r="3" stroke-width="2" fill="white" stroke="lime"/>
      <circle cx="200" cy="20" r="3" stroke-width="2" fill="white" stroke="lime"/>
      <circle cx="380" cy="20" r="3" stroke-width="2" fill="white" stroke="lime"/>
    
      <path class="path2" d="M20,40 h360 v320 h-360 v-320 Z" fill="darkblue"
            stroke="mediumseagreen" stroke-width="4"/>
      <circle cx="20"  cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="20"  cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="380" cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="380" cy="360" r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="20"  cy="360" r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="20"  cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
    </svg>
    "##,
);

test_compare_render_output!(
    image_auto_width_height,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <image
        href="data:;base64,iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAIAAAACUFjqAAAAFElEQVQY02Nk+M+ABzAxMIxKYwIAQC0BEwZFOw4AAAAASUVORK5CYII="
        x="10" y="10"/>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <rect x="10" y="10" width="10" height="10" fill="lime"/>
    </svg>"##,
);

test_compare_render_output!(
    rect_auto_width_height,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <rect x="10" y="10" width="auto" height="auto" fill="lime"/>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
    </svg>"##
);

test_compare_render_output!(
    svg_auto_width_height,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <svg xmlns="http://www.w3.org/2000/svg" width="auto" height="auto">
        <rect x="10" y="10" width="100%" height="100%" fill="lime"/>
      </svg>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <rect x="10" y="10" width="100%" height="100%" fill="lime"/>
    </svg>"##,
);

test_compare_render_output!(
    use_context_stroke,
    100,
    20,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg width="100" height="20" viewBox="0 0 40 10" xmlns="http://www.w3.org/2000/svg">
  <g id="group">
    <circle cx="5" cy="5" r="4" stroke="context-stroke" fill="black"/>
    <circle cx="14" cy="5" r="4" stroke="context-fill"/>
  </g>
  <use href="#group" x="20" stroke="blue" fill="yellow"/>
  <!--
  Modified from: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/use
  -->
</svg>
    "##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg width="100" height="20" viewBox="0 0 40 10" xmlns="http://www.w3.org/2000/svg">
    <circle cx="5" cy="5" r="4" fill="black"/>
    <circle cx="14" cy="5" r="4" fill="black"/>
    <circle cx="25" cy="5" r="4" stroke="blue" fill="black"/>
    <circle cx="34" cy="5" r="4" stroke="yellow" fill="yellow"/>
    <!--
    Modified from: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/use
    -->
  </svg>
    "##,
);

test_svg_reference!(
    isolation,
    "tests/fixtures/reftests/svg2-reftests/isolation.svg",
    "tests/fixtures/reftests/svg2-reftests/isolation-ref.svg"
);

test_svg_reference!(
    mask_and_opacity,
    "tests/fixtures/reftests/svg2-reftests/mask-and-opacity.svg",
    "tests/fixtures/reftests/svg2-reftests/mask-and-opacity-ref.svg"
);

test_svg_reference!(
    bug_880_horizontal_vertical_stroked_lines,
    "tests/fixtures/reftests/bugs-reftests/880-stroke-wide-line.svg",
    "tests/fixtures/reftests/bugs-reftests/880-stroke-wide-line-ref.svg"
);
