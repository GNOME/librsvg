use cairo;
use gio;
use glib;
use librsvg;
use rsvg_internals;

mod utils;

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use self::utils::{load_svg, render_to_viewport, test_result, SurfaceSize};

#[test]
fn render_to_viewport_with_different_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(128, 128),
        |_cr| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 128.0,
            height: 128.0,
        },
    )
    .unwrap();

    test_result(
        &output_surf,
        "render_to_viewport_with_different_size",
        "rect-48x48-rendered-128x128.png",
    );
}

#[test]
fn render_to_offsetted_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(100, 100),
        |_cr| (),
        cairo::Rectangle {
            x: 10.0,
            y: 20.0,
            width: 48.0,
            height: 48.0,
        },
    )
    .unwrap();

    test_result(
        &output_surf,
        "render_to_offseted_viewport",
        "rect-48x48-offsetted-100x100-10x20.png",
    );
}

#[test]
fn render_to_viewport_with_transform() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <g transform="translate(-10, -10)">
    <path fill="blue" d="M 18 18 l 32 0 l 0 32 l -32 0 z"/>
  </g>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(100, 100),
        |cr| cr.translate(10.0, 20.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 48.0,
            height: 48.0,
        },
    )
    .unwrap();

    test_result(
        &output_surf,
        "render_to_viewport_with_transform",
        "rect-48x48-offsetted-100x100-10x20.png",
    );
}

#[test]
fn clip_on_transformed_viewport() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <clipPath id="one" clipPathUnits="objectBoundingBox">
      <circle cx="0.5" cy="0.5" r="0.5"/>
    </clipPath>
  </defs>
  <g clip-path="url(#one)">
    <rect x="10" y="10" width="40" height="40" fill="blue"/>
    <rect x="50" y="50" width="40" height="40" fill="limegreen"/>
  </g>
</svg>
"##,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(200, 200),
        |cr| cr.translate(50.0, 50.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
    )
    .unwrap();

    test_result(
        &output_surf,
        "clip_on_transformed_viewport",
        "clip-on-transformed-viewport-200x200.png",
    );
}

#[test]
fn mask_on_transformed_viewport() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <mask id="one" maskContentUnits="objectBoundingBox">
      <circle cx="0.5" cy="0.5" r="0.5" fill="white"/>
    </mask>
  </defs>
  <g mask="url(#one)">
    <rect x="10" y="10" width="40" height="40" fill="blue"/>
    <rect x="50" y="50" width="40" height="40" fill="limegreen"/>
  </g>
</svg>
"##,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(200, 200),
        |cr| cr.translate(50.0, 50.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
    )
    .unwrap();

    test_result(
        &output_surf,
        "mask_on_transformed_viewport",
        "mask-on-transformed-viewport-200x200.png",
    );
}
