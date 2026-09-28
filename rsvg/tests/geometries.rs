//! Tests for the data files from https://github.com/horizon-eda/horizon/
//!
//! Horizon is an app Electronic Design Automation.  It has SVG templates with specially
//! named elements; the app extracts their geometries and renders GUI widgets instead of
//! those elements.  So, it is critical that the geometries get computed accurately.
//!
//! Horizon's build system pre-computes the geometries of the SVG templates' elements, and
//! stores them in JSON files.  You can see the SVGs and the .subs JSON files in the
//! tests/fixtures/horizon in the librsvg source tree.
//!
//! This test file has machinery to load the SVG templates, and the JSON files with the
//! expected geometries.  The tests check that librsvg computes the same geometries every
//! time.

use rsvg::tests_only::Rect;
use rsvg::{CairoRenderer, LengthUnit, Loader};
use serde::Deserialize;
use serde_json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

// Copy of cairo::Rectangle
//
// Somehow I can't make serde's "remote" work here, in combination with the BTreeMap below...
#[derive(Copy, Clone, Deserialize, Debug, PartialEq)]
struct Rectangle {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<Rectangle> for Rect {
    fn from(r: Rectangle) -> Rect {
        Rect {
            x0: r.x,
            y0: r.y,
            x1: r.x + r.width,
            y1: r.y + r.height,
        }
    }
}

#[derive(Deserialize)]
struct Geometries(BTreeMap<String, Rectangle>);

#[derive(Debug)]
enum ReadError {
    Io,
    Parse,
}

fn read_geometries(path: &Path) -> Result<Geometries, ReadError> {
    let contents = fs::read_to_string(path).map_err(|_| ReadError::Io)?;
    serde_json::from_str(&contents).map_err(|_| ReadError::Parse)
}

// We create a struct with the id and geometry so that
// assert_eq!() in the tests will print out the element name for failures.
#[derive(Debug, PartialEq)]
struct Element {
    id: String,
    geom: Rect,
}

macro_rules! assert_rectangles_approx_eq {
    ($id:expr, $expected:expr, $computed:expr) => {
        if !$expected.approx_eq(&$computed) {
            eprintln!(
                "assertion failed: rectangles are not approximately equal for id={}",
                $id
            );
            eprintln!("  expected: {:?}", $expected);
            eprintln!("  computed: {:?}", $computed);
            panic!();
        }
    };
}

fn test(svg_filename: &str) {
    let mut geometries_filename = String::from(svg_filename);
    geometries_filename.push_str(".subs");

    let geometries =
        read_geometries(Path::new(&geometries_filename)).expect("reading geometries JSON");

    let handle = Loader::new()
        .read_path(svg_filename)
        .expect("reading geometries SVG");
    let renderer = CairoRenderer::new(&handle);
    let dimensions = renderer.intrinsic_dimensions();
    let (svg_width, svg_height) = renderer
        .intrinsic_size_in_pixels()
        .expect("intrinsic size in pixels");

    assert!(matches!(dimensions.width.unit, LengthUnit::Px));
    assert!(matches!(dimensions.height.unit, LengthUnit::Px));
    assert_eq!(dimensions.width.length, svg_width);
    assert_eq!(dimensions.height.length, svg_height);

    for (id, expected) in geometries.0.iter() {
        println!("id: {}", id);
        let expected = Element {
            id: String::from(id),
            geom: Rect::from(*expected),
        };

        let viewport = cairo::Rectangle::new(0.0, 0.0, svg_width, svg_height);

        let (geometry, _) = renderer
            .geometry_for_layer(Some(id), &viewport)
            .unwrap_or_else(|_| panic!("getting geometry for {}", id));

        let computed = Element {
            id: String::from(id),
            geom: geometry.into(),
        };

        assert_rectangles_approx_eq!(id, expected.geom, computed.geom);
    }
}

#[test]
fn dual() {
    test("tests/fixtures/geometries/dual.svg");
}

#[test]
fn grid() {
    test("tests/fixtures/geometries/grid.svg");
}

#[test]
fn quad() {
    test("tests/fixtures/geometries/quad.svg");
}

#[test]
fn single() {
    test("tests/fixtures/geometries/single.svg");
}
