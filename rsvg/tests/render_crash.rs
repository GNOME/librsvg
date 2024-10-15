//! Tests for crashes in the rendering stage.
//!
//! Ensures that redering a particular SVG doesn't crash, but we don't care
//! about the resulting image or even whether there were errors during rendering.

use rsvg::{CairoRenderer, Loader};

use std::path::PathBuf;

fn render_crash(filename: &str) {
    let mut full_filename = PathBuf::new();
    full_filename.push("tests/fixtures/render-crash");
    full_filename.push(filename);

    let handle = Loader::new()
        .read_path(&full_filename)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
    let cr = cairo::Context::new(&surface).expect("Failed to create a cairo context");

    // We just test for crashes during rendering, and don't care about success/error.
    let _ = CairoRenderer::new(&handle)
        .render_document(&cr, &cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0));
}

macro_rules! t {
    ($test_name:ident, $filename:expr) => {
        #[test]
        fn $test_name() {
            render_crash($filename);
        }
    };
}

#[rustfmt::skip]
mod tests {
    use super::*;

    t!(bug187_set_gradient_on_empty_path_svg,           "bug187-set-gradient-on-empty-path.svg");
    t!(bug193_filters_conv_05_f_svg,                    "bug193-filters-conv-05-f.svg");
    t!(bug227_negative_dasharray_value_svg,             "bug227-negative-dasharray-value.svg");
    t!(bug266_filters_with_error_attributes_svg,        "bug266-filters-with-error-attributes.svg");
    t!(bug277_filter_on_empty_group_svg,                "bug277-filter-on-empty-group.svg");
    t!(bug292_clip_empty_group_svg,                     "bug292-clip-empty-group.svg");
    t!(bug293_mask_empty_group_svg,                     "bug293-mask-empty-group.svg");
    t!(bug324_empty_svg_svg,                            "bug324-empty-svg.svg");
    t!(bug337_font_ex_svg,                              "bug337-font-ex.svg");
    t!(bug338_zero_sized_image_svg,                     "bug338-zero-sized-image.svg");
    t!(bug340_marker_with_zero_sized_vbox_svg,          "bug340-marker-with-zero-sized-vbox.svg");
    t!(bug342_use_references_ancestor_svg,              "bug342-use-references-ancestor.svg");
    t!(bug343_fecomponenttransfer_child_in_error_svg,   "bug343-feComponentTransfer-child-in-error.svg");
    t!(bug344_too_large_viewbox_svg,                    "bug344-too-large-viewbox.svg");
    t!(bug345_too_large_size_svg,                       "bug345-too-large-size.svg");
    t!(bug395_femorphology_negative_scaling_svg,        "bug395-feMorphology-negative-scaling.svg");
    t!(bug497_path_with_all_invalid_commands_svg,       "bug497-path-with-all-invalid-commands.svg");
    t!(bug581491_zero_sized_text_svg,                   "bug581491-zero-sized-text.svg");
    t!(bug588_big_viewbox_yields_invalid_transform_svg, "bug588-big-viewbox-yields-invalid-transform.svg");
    t!(bug591_vbox_overflow_svg,                        "bug591-vbox-overflow.svg");
    t!(bug593_mask_empty_bbox_svg,                      "bug593-mask-empty-bbox.svg");
    t!(bug721_pattern_cycle_from_child_svg,             "bug721-pattern-cycle-from-child.svg");
    t!(bug721_pattern_cycle_from_other_child_svg,       "bug721-pattern-cycle-from-other-child.svg");
    t!(bug777155_zero_sized_pattern_svg,                "bug777155-zero-sized-pattern.svg");
    t!(bug928_empty_fetile_bounds_svg,                  "bug928-empty-feTile-bounds.svg");
    t!(bug932_too_big_font_size,                        "bug932-too-big-font-size.svg");
    t!(bug1059_feoffset_overflow,                       "bug1059-feoffset-overflow.svg");
    t!(bug1060_zero_sized_image_from_data_uri,          "bug1060-zero-sized-image-from-data-uri.svg");
    t!(bug1062_feturbulence_limit_numoctaves,           "bug1062-feTurbulence-limit-numOctaves.svg");
    t!(bug1088_fuzz_cairo_out_of_bounds,                "bug1088-fuzz-cairo-out-of-bounds.svg");
    t!(bug1092_fuzz_recursive_use_stack_overflow,       "bug1092-fuzz-recursive-use-stack-overflow.svg");
    t!(bug1100_fuzz_layer_nesting_depth,                "bug1100-fuzz-layer-nesting-depth.svg");
    t!(bug1115_feturbulence_overflow,                   "bug1115-feTurbulence-overflow.svg");
    t!(bug1118_fuzz_large_transform_and_recursive_use,  "bug1118-fuzz-large-transform-and-recursive-use.svg");
    t!(femerge_color_interpolation_srgb_svg,            "feMerge-color-interpolation-srgb.svg");
    t!(filters_non_invertible_paffine_svg,              "filters-non-invertible-paffine.svg");
    t!(gradient_with_empty_bbox_svg,                    "gradient-with-empty-bbox.svg");
    t!(gradient_with_no_children_svg,                   "gradient-with-no-children.svg");
    t!(pattern_with_empty_bbox_svg,                     "pattern-with-empty-bbox.svg");
    t!(pattern_with_no_children_svg,                    "pattern-with-no-children.svg");
    t!(pixelrectangle_duplicate_crash_svg,              "PixelRectangle-duplicate-crash.svg");
    t!(recursive_feimage_svg,                           "recursive-feimage.svg");
}
