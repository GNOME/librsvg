//! Tests for geometries of SVG primitives
//!
//! These use the *.svg and *.svg.geom files in the tests/fixtures/primitive_geometries directory.
//!
//! Each .svg.geom is a JSON file formatted like this:
//!
//! ```json
//! {
//!     "#element_id": {
//!         "ink_rect": {
//!             "x": 5.0,
//!             "y": 15.0,
//!             "width": 40.0,
//!             "height": 50.0
//!         },
//!         "logical_rect": {
//!             "x": 10.0,
//!             "y": 20.0,
//!             "width": 30.0,
//!             "height": 40.0
//!         }
//!     }
//! }
//! ```
//!
//! Any number of element_ids may appear in the file.  For each of those, the `test()` function will
//! call `CairoRenderer::get_layer_geometry()` and compare its result against the provided rectangles.

use rsvg::tests_only::Rect;
use rsvg::{CairoRenderer, LengthUnit, Loader};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

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

#[derive(Copy, Clone, Deserialize, Debug, PartialEq)]
struct ElementGeometry {
    ink_rect: Option<Rectangle>,
    logical_rect: Option<Rectangle>,
}

#[derive(Deserialize)]
struct Geometries(BTreeMap<String, ElementGeometry>);

#[derive(Debug)]
enum ReadError {
    Io,
    Parse,
}

fn read_geometries(path: &str) -> Result<Geometries, ReadError> {
    let contents = fs::read_to_string(path).map_err(|_| ReadError::Io)?;
    serde_json::from_str(&contents).map_err(|_| ReadError::Parse)
}

// We create a struct with the id and geometry so that
// assert_eq!() in the tests will print out the element name for failures.
//
// Here we use rsvg::Rect as that one has an approx_eq() method.
#[derive(Debug, PartialEq)]
struct Element {
    id: String,
    ink_rect: Option<Rect>,
    logical_rect: Option<Rect>,
}

impl Element {
    fn from_element_geometry(id: &str, geom: &ElementGeometry) -> Element {
        Element {
            id: String::from(id),
            ink_rect: geom.ink_rect.map(Into::into),
            logical_rect: geom.logical_rect.map(Into::into),
        }
    }

    fn from_rectangles(
        id: &str,
        ink_rect: cairo::Rectangle,
        logical_rect: cairo::Rectangle,
    ) -> Element {
        Element {
            id: String::from(id),
            ink_rect: Some(ink_rect.into()),
            logical_rect: Some(logical_rect.into()),
        }
    }
}

fn test(svg_filename: &str, geometries_filename: &str) {
    let geometries = read_geometries(geometries_filename).expect("reading geometries JSON");

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
        let expected = Element::from_element_geometry(id, expected);

        let viewport = cairo::Rectangle::new(0.0, 0.0, svg_width, svg_height);

        let (ink_rect, logical_rect) = renderer
            .geometry_for_layer(Some(id), &viewport)
            .unwrap_or_else(|_| panic!("getting geometry for {}", id));

        let computed = Element::from_rectangles(id, ink_rect, logical_rect);

        assert_eq!(expected, computed);
    }
}

#[test]
fn rect() {
    test(
        "tests/fixtures/primitive_geometries/rect.svg",
        "tests/fixtures/primitive_geometries/rect.svg.geom",
    );
}

#[test]
fn rect_stroke() {
    test(
        "tests/fixtures/primitive_geometries/rect_stroke.svg",
        "tests/fixtures/primitive_geometries/rect_stroke.svg.geom",
    );
}

#[test]
fn rect_stroke_unfilled() {
    test(
        "tests/fixtures/primitive_geometries/rect_stroke_unfilled.svg",
        "tests/fixtures/primitive_geometries/rect_stroke_unfilled.svg.geom",
    );
}

#[test]
fn rect_isolate() {
    test(
        "tests/fixtures/primitive_geometries/rect_isolate.svg",
        "tests/fixtures/primitive_geometries/rect_isolate.svg.geom",
    );
}
