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
