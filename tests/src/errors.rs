//! Tests for loading errors.

#![cfg(test)]

use cairo;
use librsvg::{CairoRenderer, Loader, LoadingError, RenderingError};

use crate::utils::fixture_path;

#[test]
fn too_many_elements() {
    let name = "tests/fixtures/errors/515-too-many-elements.svgz";

    assert!(matches!(
        Loader::new().read_path(fixture_path(name)),
        Err(LoadingError::XmlParseError(_))
    ));
}

fn rendering_instancing_limit(name: &str) {
    let handle = Loader::new()
        .read_path(fixture_path(name))
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();
    let cr = cairo::Context::new(&surface);

    // Note that at least 515-patttern-billion-laughs.svg requires a viewport of this size
    // or bigger; a smaller one causes the recursive patterns to get so small that they
    // are culled out, and so the document doesn't reach the instancing limit.
    assert!(matches!(
        CairoRenderer::new(&handle).render_document(
            &cr,
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 500.0,
                height: 500.0,
            },
        ),
        Err(RenderingError::InstancingLimit)
    ));
}

#[test]
fn instancing_limit1() {
    rendering_instancing_limit("tests/fixtures/errors/323-nested-use.svg");
}

#[test]
fn instancing_limit2() {
    rendering_instancing_limit("tests/fixtures/errors/515-pattern-billion-laughs.svg");
}
