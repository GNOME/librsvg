use cairo;
use float_cmp::approx_eq;
use librsvg::{CairoRenderer, Loader};
use test_generator::test_resources;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::utils::{load_svg, render_document, setup_font_map, SurfaceSize};
use crate::{test_compare_render_output, test_svg_reference};

// From https://www.w3.org/Style/CSS/Test/Fonts/Ahem/
//
//   > The Ahem font was developed by Todd Fahrner and Myles C. Maxfield to
//   > help test writers develop predictable tests. The units per em is 1000,
//   > the ascent is 800, and the descent is 200, thereby making the em
//   > square exactly square. The glyphs for most characters is simply a box
//   > which fills this square. The codepoints mapped to this full square
//   > with a full advance are the following ranges:
//
// So, ascent is 4/5 of the font-size, descent is 1/5.  Mind the positions below.
test_compare_render_output!(
    ahem_font,
    500,
    500,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="500" height="500">
  <text style="font: 50px Ahem;" x="50" y="50" fill="black">abcde</text>
</svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="500" height="500">
  <rect x="50" y="10" width="250" height="50" fill="black"/>
</svg>"##,
);

test_svg_reference!(
    text_anchor_chunk_806,
    "tests/fixtures/text/806-text-anchor-chunk.svg",
    "tests/fixtures/text/806-text-anchor-chunk-ref.svg"
);

test_svg_reference!(
    span_bounds_when_offset_by_dx,
    "tests/fixtures/text/span-bounds-when-offset-by-dx.svg",
    "tests/fixtures/text/span-bounds-when-offset-by-dx-ref.svg"
);

// FIXME: Ignored because with the change to render all text as paths, the rendering
// of these is different.
//
// test_svg_reference!(
//     tspan_direction_change_804,
//     "tests/fixtures/text/804-tspan-direction-change.svg",
//     "tests/fixtures/text/804-tspan-direction-change-ref.svg"
// );

test_svg_reference!(
    unicode_bidi_override,
    "tests/fixtures/text/unicode-bidi-override.svg",
    "tests/fixtures/text/unicode-bidi-override-ref.svg"
);

test_svg_reference!(
    display_none,
    "tests/fixtures/text/display-none.svg",
    "tests/fixtures/text/display-none-ref.svg"
);

test_svg_reference!(
    visibility_hidden,
    "tests/fixtures/text/visibility-hidden.svg",
    "tests/fixtures/text/visibility-hidden-ref.svg"
);

test_svg_reference!(
    visibility_hidden_x_attr,
    "tests/fixtures/text/visibility-hidden-x-attr.svg",
    "tests/fixtures/text/visibility-hidden-ref.svg"
);

test_svg_reference!(
    bounds,
    "tests/fixtures/text/bounds.svg",
    "tests/fixtures/text/bounds-ref.svg"
);

fn rect(x: f64, y: f64, width: f64, height: f64) -> cairo::Rectangle {
    cairo::Rectangle {
        x,
        y,
        width,
        height,
    }
}

fn rectangle_approx_eq(a: &cairo::Rectangle, b: &cairo::Rectangle) -> bool {
    // FIXME: this is super fishy; shouldn't we be using 2x the epsilon against the width/height
    // instead of the raw coordinates?
    approx_eq!(f64, a.x, b.x)
        && approx_eq!(f64, a.y, b.y)
        && approx_eq!(f64, a.width, b.width)
        && approx_eq!(f64, a.height, b.height)
}

// Test that the computed geometry of text layers is as expected.
#[test_resources("tests/fixtures/text/bounds.svg")]
fn text_layer_geometry(name: &str) {
    setup_font_map();

    let handle = Loader::new()
        .read_path(name)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let renderer = CairoRenderer::new(&handle).test_mode(true);

    let viewport = rect(0.0, 0.0, 600.0, 600.0);

    // tuples of (element_id, ink_rect)
    let cases = vec![
        ("#a", rect(50.0, 60.0, 100.0, 50.0)),
        ("#b", rect(200.0, 60.0, 50.0, 100.0)),
        ("#c", rect(300.0, 60.0, 50.0, 100.0)),
        ("#d", rect(400.0, 60.0, 100.0, 50.0)),
    ];

    for (id, expected_ink_rect) in cases {
        let (ink_rect, _) = renderer.geometry_for_layer(Some(id), &viewport).unwrap();
        assert!(
            rectangle_approx_eq(&ink_rect, &expected_ink_rect),
            "ink_rect: {:?}, expected: {:?}",
            ink_rect,
            expected_ink_rect
        );
    }
}
