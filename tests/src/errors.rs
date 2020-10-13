//! Tests for loading errors.

#![cfg(test)]
use test_generator::test_resources;

use librsvg::{Loader, LoadingError};

use crate::utils::fixture_path;

#[test_resources("tests/fixtures/errors/515-too-many-elements.svgz")]
fn loading_crash(name: &str) {
    assert!(matches!(
        Loader::new().read_path(fixture_path(name)),
        Err(LoadingError::XmlParseError(_))
    ));
}
