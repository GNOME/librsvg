use cairo;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::utils::{load_svg, render_document, SurfaceSize};
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

test_svg_reference!(
    tspan_direction_change_804,
    "tests/fixtures/text/804-tspan-direction-change.svg",
    "tests/fixtures/text/804-tspan-direction-change-ref.svg"
);

test_svg_reference!(
    unicode_bidi_override,
    "tests/fixtures/text/unicode-bidi-override.svg",
    "tests/fixtures/text/unicode-bidi-override-ref.svg"
);
