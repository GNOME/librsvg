use cairo;

use crate::reference_utils::{Compare, Evaluate, Reference};
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
        let cr = cairo::Context::new(&reference_surf);

        cr.rectangle(100.0, 100.0, 200.0, 200.0);
        cr.set_source_rgb(0.0, 1.0, 0.0);
        cr.fill();
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
        let cr = cairo::Context::new(&reference_surf);

        cr.rectangle(100.0, 100.0, 200.0, 200.0);
        cr.set_source_rgb(0.0, 1.0, 0.0);
        cr.fill();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "non_filter_reference_cancels_filter_chain");
}

#[test]
fn blur_filter_func() {
    // Create an element with a filter function, and compare it to the
    // supposed equivalent using the <filter> element.
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="400">
  <rect x="100" y="100" width="200" height="200" fill="lime" filter="blur(5)"/>
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

    let reference = load_svg(
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
    ).unwrap();

    let reference_surf = render_document(
        &reference,
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

    Reference::from_surface(reference_surf.into_image_surface().unwrap())
        .compare(&output_surf)
        .evaluate(&output_surf, "blur_filter_func");
}
