#![cfg(test)]

use std::env;
use std::path::PathBuf;

/// Given a filename from `test_generator::test_resources`, computes the correct fixture filename.
///
/// The `test_resources` procedural macro works by running a filename glob starting on
/// the toplevel of the Cargo workspace.  However, when a test function gets run,
/// its $cwd is the test crate's toplevel.  This function fixes the pathname generated
/// by `test_resources` so that it has the correct path.
pub fn fixture_path(filename_from_test_resources: &str) -> PathBuf {
    let crate_toplevel = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .expect(r#"CARGO_MANIFEST_DIR" is not set, please set it or run under "cargo test""#),
    );

    let workspace_toplevel = crate_toplevel.parent().unwrap();

    workspace_toplevel.join(filename_from_test_resources)
}

#[cfg(have_pangoft2)]
pub fn setup_font_map() {
    use glib::prelude::*;
    use pangocairo::FontMap;

    let font_map = FontMap::new_for_font_type(cairo::FontType::FontTypeFt);

    // TODO: create and apply FontConfig

    FontMap::set_default(font_map.map(|m| m.downcast::<pangocairo::FontMap>().unwrap()));
}

#[cfg(not(have_pangoft2))]
pub fn setup_font_map() {}
