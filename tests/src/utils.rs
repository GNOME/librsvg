#![cfg(test)]

use glib::translate::*;
use libc;
use std::env;
use std::ffi::CString;
use std::sync::Once;

#[cfg(have_pangoft2)]
mod pango_ft2 {
    use super::*;
    use fontconfig_sys::fontconfig;
    use glib::prelude::*;
    use pangocairo::FontMap;

    extern "C" {
        // pango_fc_font_map_set_config (PangoFcFontMap *fcfontmap,
        //                               FcConfig       *fcconfig);
        // This is not bound in gtk-rs, and PangoFcFontMap is not even exposed, so we'll bind it by hand.
        fn pango_fc_font_map_set_config(
            font_map: *mut libc::c_void,
            config: *mut fontconfig::FcConfig,
        );
    }

    pub unsafe fn load_test_fonts() {
        let font_paths = [
            "tests/resources/Roboto-Regular.ttf",
            "tests/resources/Roboto-Italic.ttf",
            "tests/resources/Roboto-Bold.ttf",
            "tests/resources/Roboto-BoldItalic.ttf",
        ];

        let config = fontconfig::FcConfigCreate();

        for path in &font_paths {
            let path_cstring = CString::new(*path).unwrap();

            if fontconfig::FcConfigAppFontAddFile(config, path_cstring.as_ptr() as *const _) == 0 {
                panic!(
                    "Could not load font file {} for tests; aborting",
                    path,
                );
            }
        }

        let font_map = FontMap::new_for_font_type(cairo::FontType::FontTypeFt).unwrap();
        let raw_font_map: *mut pango_sys::PangoFontMap = font_map.to_glib_none().0;

        pango_fc_font_map_set_config(raw_font_map as *mut _, config);
        fontconfig::FcConfigDestroy(config);

        FontMap::set_default(Some(font_map.downcast::<pangocairo::FontMap>().unwrap()));
    }
}

#[cfg(have_pangoft2)]
pub fn setup_font_map() {
    unsafe {
        self::pango_ft2::load_test_fonts();
    }
}

#[cfg(not(have_pangoft2))]
pub fn setup_font_map() {}

pub fn setup_language() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        // For systemLanguage attribute tests.
        // The trailing ":" is intentional to test gitlab#425.
        env::set_var("LANGUAGE", "de:en_US:en:");
        env::set_var("LC_ALL", "de:en_US:en:");
    });
}
