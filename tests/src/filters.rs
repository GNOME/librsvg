use cairo;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::test_compare_render_output;
use crate::utils::{load_svg, render_document, SurfaceSize};

#[test]
fn invalid_filter_reference_cancels_filter_chain() {
    // The <rect> has a filter chain with two URLs listed, but the second one doesn't resolve.
    // The whole filter chain should be ignored.
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feColorMatrix type="hueRotate" values="240"/>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter) url(#nonexistent)"/>
</svg>
"##,
    ).unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(400, 400),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 400.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 400).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.rectangle(100.0, 100.0, 200.0, 200.0);
        cr.set_source_rgb(0.0, 1.0, 0.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(
            &output_surf,
            "invalid_filter_reference_cancels_filter_chain",
        );
}

#[test]
fn non_filter_reference_cancels_filter_chain() {
    // The <rect> has a filter chain, but one of the URLs does not point to a <filter>.
    // The whole filter chain should be ignored.
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feColorMatrix type="hueRotate" values="240"/>
    </filter>
    <g id="not_a_filter"/>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter) url(#not_a_filter)"/>
</svg>
"##,
    ).unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(400, 400),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 400.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 400).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.rectangle(100.0, 100.0, 200.0, 200.0);
        cr.set_source_rgb(0.0, 1.0, 0.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "non_filter_reference_cancels_filter_chain");
}

test_compare_render_output!(
    blur_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="blur(5)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feGaussianBlur stdDeviation="5 5" edgeMode="none"/>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    brightness_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="green" filter="brightness(125%)"/>
</svg>
"##,
br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feComponentTransfer>
        <feFuncR type="linear" slope="1.25" />
        <feFuncG type="linear" slope="1.25" />
        <feFuncB type="linear" slope="1.25" />
      </feComponentTransfer>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="green" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    contrast_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="green" filter="contrast(125%)"/>
</svg>
"##,
br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feComponentTransfer>
        <feFuncR type="linear" slope="1.25" intercept="-0.125" />
        <feFuncG type="linear" slope="1.25" intercept="-0.125" />
        <feFuncB type="linear" slope="1.25" intercept="-0.125" />
      </feComponentTransfer>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="green" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    dropshadow_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="green" filter="drop-shadow(#ff0000 1px 4px 6px)"/>
</svg>
"##,
br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feGaussianBlur in="SourceAlpha" stdDeviation="6" />
      <feOffset dx="1" dy="4" result="offsetblur" />
      <feFlood flood-color="#ff0000" />
      <feComposite in2="offsetblur" operator="in" />
      <feMerge>
        <feMergeNode />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="green" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    grayscale_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="grayscale(0.75)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feColorMatrix type="saturate" values="0.25" />
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    huerotate_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="green" filter="hue-rotate(128deg)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feColorMatrix type="hueRotate" values="128" />
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="green" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    invert_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="invert(0.75)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feComponentTransfer>
        <feFuncR type="table" tableValues="0.75 0.25" />
        <feFuncG type="table" tableValues="0.75 0.25" />
        <feFuncB type="table" tableValues="0.75 0.25" />
      </feComponentTransfer>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    opacity_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="red"/>
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="opacity(0.75)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feComponentTransfer>
        <feFuncA type="table" tableValues="0 0.75" />
      </feComponentTransfer>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="red"/>
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter)"/>
</svg>
"##
);

test_compare_render_output!(
    saturate_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="saturate(0.75)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feColorMatrix type="saturate" values="0.75" />
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter)"/>
</svg>
"##,
);

test_compare_render_output!(
    sepia_filter_func,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="sepia(0.75)"/>
</svg>
"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <defs>
    <filter id="filter">
      <feColorMatrix type="matrix"
         values="0.5447500000000001 0.57675 0.14175 0 0
                 0.26175 0.7645000000000001 0.126 0 0
                 0.20400000000000001 0.4005 0.34825 0 0
                 0 0 0 1 0"/>
    </filter>
  </defs>

  <rect x="100" y="100" width="200" height="200" fill="lime" filter="url(#filter)"/>
</svg>
"##,
);
