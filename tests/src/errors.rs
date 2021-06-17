//! Tests for loading errors.
//!
//! Note that all the tests in this module are `#[ignore]`.  This is because they
//! take a much longer time to run than normal tests, as they depend upon actually
//! hitting the limits in librsvg for the number of loaded elements, or the number
//! of referenced elements during rendering.
//!
//! There is a *big* difference in the run-time of these tests when compiled with
//! `--release` versus `--debug`.  So, we will only run them in release-mode tests.

#![cfg(test)]

use cairo;
use librsvg::{CairoRenderer, ImplementationLimit, Loader, LoadingError, RenderingError};

#[ignore]
#[test]
fn too_many_elements() {
    let name = "tests/fixtures/errors/515-too-many-elements.svgz";

    assert!(matches!(
        Loader::new().read_path(name),
        Err(LoadingError::LimitExceeded(
            ImplementationLimit::TooManyLoadedElements
        ))
    ));
}

fn rendering_instancing_limit(name: &str) {
    let handle = Loader::new()
        .read_path(name)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();
    let cr = cairo::Context::new(&surface).expect("Failed to create a cairo context");

    // Note that at least 515-patttern-billion-laughs.svg requires a viewport of this size
    // or bigger; a smaller one causes the recursive patterns to get so small that they
    // are culled out, and so the document doesn't reach the instancing limit.
    match CairoRenderer::new(&handle).render_document(
        &cr,
        &cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 500.0,
        },
    ) {
        Ok(_) => (),
        Err(RenderingError::LimitExceeded(ImplementationLimit::TooManyReferencedElements)) => (),
        _ => panic!("unexpected error code"),
    }
}

#[ignore]
#[test]
fn instancing_limit1() {
    rendering_instancing_limit("tests/fixtures/errors/323-nested-use.svg");
}

#[ignore]
#[test]
fn instancing_limit2() {
    rendering_instancing_limit("tests/fixtures/errors/515-pattern-billion-laughs.svg");
}
