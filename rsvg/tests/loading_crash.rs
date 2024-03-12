//! Tests for crashes in the loading stage.
//!
//! Ensures that loading and parsing (but not rendering) a particular
//! SVG doesn't crash.

use rsvg::Loader;

use std::path::PathBuf;

fn loading_crash(filename: &str) {
    let mut full_filename = PathBuf::new();
    full_filename.push("tests/fixtures/crash");
    full_filename.push(filename);

    // We just test for crashes during loading, and don't care about success/error.
    let _ = Loader::new().read_path(&full_filename);
}

macro_rules! t {
    ($test_name:ident, $filename:expr) => {
        #[test]
        fn $test_name() {
            loading_crash($filename);
        }
    };
}

#[rustfmt::skip]
mod tests {
    use super::*;

    t!(bug335_non_svg_toplevel_svg,                  "bug335-non-svg-toplevel.svg");
    t!(bug336_invalid_css_svg,                       "bug336-invalid-css.svg");
    t!(bug349_empty_data_uri_svg,                    "bug349-empty-data-uri.svg");
    t!(bug349_too_big_image_in_href_data_svg,        "bug349-too-big-image-in-href-data.svg");
    t!(bug352_feconvolvematrix_large_allocation_svg, "bug352-feConvolveMatrix-large-allocation.svg");
    t!(bug377_xinclude_invalid_xml_svg,              "bug377-xinclude-invalid-xml.svg");
    t!(bug463_characters_outside_first_element_svg,  "bug463-characters-outside-first-element.svg");
    t!(bug467_xinclude_without_parent_element_svg,   "bug467-xinclude-without-parent-element.svg");
    t!(bug524_invalid_stylesheet_href_svg,           "bug524-invalid-stylesheet-href.svg");
    t!(bug942_xinclude_recursion_svg,                "bug942-xinclude-recursion.svg");
    t!(bug942_xinclude_mutual_recursion_svg,         "bug942-xinclude-mutual-recursion.svg");
    t!(bug620238_svg,                                "bug620238.svg");
    t!(bug759084_svg,                                "bug759084.svg");
    t!(bug785276_empty_svg,                          "bug785276-empty.svg");
    t!(bug785276_short_file_svg,                     "bug785276-short-file.svg");
    t!(bug800_font_inherit_svg,                      "bug800-font-inherit.svg");
    t!(bug800_marker_svg,                            "bug800-marker.svg");
    t!(bug1064_private_lang_tag_in_lang_selector,    "bug1064-private-lang-tag-in-lang-selector.svg");
    t!(feconvolvematrix_empty_kernel_svg,            "feConvolveMatrix-empty-kernel.svg");
    t!(marker_cycles_svg,                            "marker-cycles.svg");
    t!(mask_cycles_svg,                              "mask-cycles.svg");
    t!(pattern_fallback_cycles_svg,                  "pattern-fallback-cycles.svg");
    t!(xinclude_text_xml_svg,                        "xinclude-text-xml.svg");
    t!(xml_pi_without_data_svg,                      "xml-pi-without-data.svg");
}
