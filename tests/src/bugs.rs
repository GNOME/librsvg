#![cfg(test)]
use test_generator::test_resources;

use cairo;
use librsvg::{CairoRenderer, Loader, LoadingError, SvgHandle};
use matches::matches;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::utils::{load_svg, render_document, setup_font_map, setup_language, SurfaceSize};

// https://gitlab.gnome.org/GNOME/librsvg/issues/335
#[test]
fn non_svg_root() {
    assert!(matches!(load_svg(b"<x></x>"), Err(LoadingError::NoSvgRoot)));
}

// https://gitlab.gnome.org/GNOME/librsvg/issues/496
#[test]
fn inf_width() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg s="Pg" width="1001111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111" heiNht=" 00">
 [l<g mask="url(sHaf:ax-fwiw0\inside\ax-ide\ax-flow#o0" styli="fility:!.5;">>
  </g>
</svg>"#,
    ).unwrap();

    let _output_surf = render_document(
        &svg,
        SurfaceSize(150, 150),
        |cr| cr.translate(50.0, 50.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
    )
    .unwrap();
}

// https://gitlab.gnome.org/GNOME/librsvg/issues/547
#[test]
fn nonexistent_image_shouldnt_cancel_rendering() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     width="50" height="50">
  <image xlink:href="nonexistent.png" width="10" height="10"/>
  <rect x="10" y="10" width="30" height="30" fill="blue"/>
</svg>
"#,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(50, 50),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 50, 50).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).unwrap();

        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "nonexistent_image_shouldnt_cancel_rendering");
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/568
#[test]
fn href_attribute_overrides_xlink_href() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:xlink="http://www.w3.org/1999/xlink"
     width="500" height="500">
  <defs>
    <rect id="one" x="100" y="100" width="100" height="100" fill="red"/>
    <rect id="two" x="100" y="100" width="100" height="100" fill="lime"/>
  </defs>

  <!-- Per https://svgwg.org/svg2-draft/linking.html#XLinkRefAttrs a plain
       href attribute overrides an xlink:href one in SVG2 -->
  <use xlink:href="#one" href="#two"/>
</svg>
"##,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(500, 500),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 500.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).unwrap();

        cr.rectangle(100.0, 100.0, 100.0, 100.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "href_attribute_overrides_xlink_href");
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/560
#[test]
fn nonexistent_filter_leaves_object_unfiltered() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     width="500" height="500">
  <rect x="100" y="100" width="100" height="100" fill="lime" filter="url(#nonexistent)"/>
</svg>
"##,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(500, 500),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 500.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).unwrap();

        cr.rectangle(100.0, 100.0, 100.0, 100.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "nonexistent_filter_leaves_object_unfiltered");
}

// https://www.w3.org/TR/SVG2/painting.html#SpecifyingPaint says this:
//
// A <paint> allows a paint server reference, to be optionally
// followed by a <color> or the keyword none. When this optional value
// is given, the <color> value or the value none is a fallback value
// to use if the paint server reference in the layer is invalid (due
// to pointing to an element that does not exist or which is not a
// valid paint server).
//
// I'm interpreting this to mean that if we have
// fill="url(#recursive_paint_server) fallback_color", then the
// recursive paint server is not valid, and should fall back to to the
// specified color.
#[test]
fn recursive_paint_servers_fallback_to_color() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     width="200" height="200">
  <defs>
    <pattern id="p" width="10" height="10" xlink:href="#p"/>
    <linearGradient id="l" xlink:href="#r"/>
    <radialGradient id="r" xlink:href="#l"/>
  </defs>

  <!-- These two should not render as there is no fallback color -->
  <rect fill="url(#p)" x="0" y="0" width="100" height="100" />
  <rect fill="url(#l)" x="100" y="0" width="100" height="100" />

  <!-- These two should render with the fallback color -->
  <rect fill="url(#p) lime" x="0" y="100" width="100" height="100" />
  <rect fill="url(#l) lime" x="100" y="100" width="100" height="100" />
</svg>
"##,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(200, 200),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).unwrap();

        cr.rectangle(0.0, 100.0, 200.0, 100.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "recursive_paint_servers_fallback_to_color");
}

fn test_renders_as_empty(svg: &SvgHandle, test_name: &str) {
    let output_surf = render_document(
        &svg,
        SurfaceSize(100, 100),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, test_name);
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/308
#[test]
fn recursive_use() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns:xlink="http://www.w3.org/1999/xlink">
  <defs>
    <g id="one">
      <use xlink:href="#one"/>
    </g>
  </defs>

  <use xlink:href="#one"/>
</svg>
"##,
    )
    .unwrap();

    test_renders_as_empty(&svg, "308-recursive-use");
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/308
#[test]
fn use_self_ref() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns:xlink="http://www.w3.org/1999/xlink">
  <defs>
    <use id="one" xlink:href="#one"/>
  </defs>

  <use xlink:href="#one"/>
</svg>
"##,
    )
    .unwrap();

    test_renders_as_empty(&svg, "308-use-self-ref");
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/308
#[test]
fn doubly_recursive_use() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns:xlink="http://www.w3.org/1999/xlink">
  <defs>
    <g id="one">
      <use xlink:href="#two"/>
    </g>

    <g id="two">
      <use xlink:href="#one"/>
    </g>
  </defs>

  <use xlink:href="#one"/>
</svg>
"##,
    )
    .unwrap();

    test_renders_as_empty(&svg, "308-doubly-recursive-use");
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/347
#[test_resources("tests/fixtures/dimensions/347-wrapper.svg")]
fn test_text_bounds(name: &str) {
    setup_font_map();

    let handle = Loader::new()
        .read_path(name)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let renderer = CairoRenderer::new(&handle).test_mode(true);

    let (ink_r, _) = renderer
        .geometry_for_layer(
            Some("#LabelA"),
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 248.0,
                height: 176.0,
            },
        )
        .unwrap();

    assert!(ink_r.x >= 80.0 && ink_r.x < 80.1);

    // This is kind of suspicious, but we don't know the actual height of the
    // text set at y=49 in the test SVG.  However, this test is more "text
    // elements compute sensible bounds"; the bug #347 was that their ink_rect
    // was not being computed correctly at all.
    assert!(ink_r.y > 48.0 && ink_r.y < 49.0);
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/703
#[test]
fn switch_element_should_ignore_elements_in_error() {
    setup_language();

    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <switch>
    <rect x="10" y="10" width="10" height="10" systemLanguage="es_MX" id="es" fill="red"/>
    <rect x="10" y="10" width="10" height="10" id="no_lang" fill="blue"/>
  </switch>
</svg>
"##,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(100, 100),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).unwrap();

        cr.rectangle(10.0, 10.0, 10.0, 10.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(
            &output_surf,
            "switch_element_should_ignore_elements_in_error",
        );
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/566
#[test]
fn accepted_children_inside_clip_path() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="200" height="200">
  <defs>
    <clipPath id="one">
      <g>
        <rect x="10" y="10" width="50" height="50"/>
      </g>
    </clipPath>

    <clipPath id="two">
      <use xlink:href="#three"/>
    </clipPath>

    <use id="three" xlink:href="#four"/>

    <rect id="four" x="10" y="10" width="50" height="50"/>
  </defs>

  <rect x="10" y="10" width="100" height="100" fill="lime"/>

  <rect x="20" y="20" width="10" height="10" fill="red" clip-path="url(#one)"/>

  <rect x="40" y="40" width="10" height="10" fill="red" clip-path="url(#two)"/>
</svg>
"##,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(200, 200),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).unwrap();

        cr.rectangle(10.0, 10.0, 100.0, 100.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "accepted_children_inside_clip_path");
}

#[test]
fn can_draw_to_non_image_surface() {
    // This tries to exercise the various tricky code paths in DrawingCtx::with_discrete_layer()
    // that depend on whether there are filter/masks/opacity - they are easy to break when
    // the application is using something other than a cairo::ImageSurface.
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="400" height="100">
  <!-- code path with opacity, no mask -->
  <rect x="0" y="0" width="100" height="100" fill="lime" opacity="0.5"/>

  <!-- code path with mask -->
  <mask id="mask" maskUnits="objectBoundingBox">
    <rect x="10%" y="10%" width="80%" height="80%" fill="white"/>
  </mask>
  <rect x="100" y="0" width="100" height="100" fill="lime" mask="url(#mask)"/>

  <!-- code path with filter -->
  <rect x="200" y="0" width="100" height="100" fill="lime" filter="blur(5)"/>

  <!-- code path with filter and mask-->
  <rect x="300" y="0" width="100" height="100" fill="lime" filter="blur(5)" mask="url(#mask)"/>
</svg>
"##,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 200.0,
    };

    let output =
        cairo::RecordingSurface::create(cairo::Content::ColorAlpha, Some(viewport)).unwrap();

    let cr = cairo::Context::new(&output).expect("Failed to create a cairo context");
    renderer
        .render_document(&cr, &viewport)
        .expect("Failed to render to non-image surface");
}
