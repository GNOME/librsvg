#![cfg(test)]

use std::env;
use std::path::PathBuf;

#[cfg(have_pangoft2)]

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
    use fontconfig_sys::fontconfig;
    use glib::prelude::*;
    use pangocairo::FontMap;

    let font_paths = [
        "tests/resources/Roboto-Regular.ttf",
        "tests/resources/Roboto-Italic.ttf",
        "tests/resources/Roboto-Bold.ttf",
        "tests/resources/Roboto-BoldItalic.ttf",
    ];

    let config = unsafe { fontconfig::FcConfigCreate() };

    for path in &font_paths {
        let path = fixture_path(path);
        let str = path.to_str().unwrap();

        unsafe {
            if fontconfig::FcConfigAppFontAddFile(config, str.as_ptr()) == 0 {
                panic!("Could not load font file {} for tests; aborting", str);
            }
        };
    }

    let font_map = FontMap::new_for_font_type(cairo::FontType::FontTypeFt);

    // TODO: apply config
    unsafe {
        fontconfig::FcConfigDestroy(config);
    };

    FontMap::set_default(font_map.map(|m| m.downcast::<pangocairo::FontMap>().unwrap()));
}

#[cfg(not(have_pangoft2))]
pub fn setup_font_map() {}
