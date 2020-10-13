//! Tests for crashes in the loading stage.
//!
//! Ensures that loading and parsing (but not rendering) a particular
//! SVG doesn't crash.

#![cfg(test)]
use test_generator::test_resources;

use librsvg::Loader;

use crate::utils::fixture_path;

#[test_resources("tests/fixtures/crash/*.svg")]
fn loading_crash(name: &str) {
    let path = fixture_path(name);

    // We just test for crashes during loading, and don't care about success/error.
    let _ = Loader::new().read_path(path);
}
