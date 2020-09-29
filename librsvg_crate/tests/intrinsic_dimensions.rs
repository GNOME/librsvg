use cairo;

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use librsvg::{
    CairoRenderer, DefsLookupErrorKind, HrefError, IntrinsicDimensions, Length, LengthUnit,
    RenderingError,
};

mod utils;

use self::utils::{compare_to_surface, load_svg, render_document, SurfaceSize};

#[test]
fn no_intrinsic_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"/>
"#,
    );

    assert_eq!(
        CairoRenderer::new(&svg).intrinsic_dimensions(),
        IntrinsicDimensions {
            width: None,
            height: None,
            vbox: None,
        }
    );
}

#[test]
fn has_intrinsic_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="10cm" height="20" viewBox="0 0 100 200"/>
"#,
    );

    assert_eq!(
        CairoRenderer::new(&svg).intrinsic_dimensions(),
        IntrinsicDimensions {
            width: Some(Length::new(10.0, LengthUnit::Cm)),
            height: Some(Length::new(20.0, LengthUnit::Px)),
            vbox: Some(cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 200.0,
            }),
        }
    );
}

#[test]
fn root_geometry_with_percent_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect x="10" y="20" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_with_offset_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect x="10" y="20" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle {
        x: 110.0,
        y: 120.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_with_viewbox_and_offset_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="100 100 100 100">
  <rect x="110" y="120" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle {
        x: 110.0,
        y: 120.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_with_no_width_height() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="100 100 200 200">
  <rect x="110" y="120" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle {
        x: 105.0,
        y: 110.0,
        width: 15.0,
        height: 20.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_with_no_intrinsic_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <rect x="110" y="120" width="50" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    // The SVG document above has no width/height nor viewBox, which means it should
    // start with an identity transform for its coordinate space.  Since the viewport
    // is just offset by (100, 100), this just translates the coordinates of the <rect>.
    let rect = cairo::Rectangle {
        x: 210.0,
        y: 220.0,
        width: 50.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_with_percentage_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="50%">
  <rect x="10" y="20" width="50" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    // Lack of viewBox means we use an identity transform, so the <rect> is just
    // offset by (100, 100) because of the viewport.
    let rect = cairo::Rectangle {
        x: 110.0,
        y: 120.0,
        width: 50.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_with_percent_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect id="foo" x="10" y="20" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    let (ink_r, logical_r) = renderer
        .geometry_for_layer(Some("#foo"), &viewport)
        .unwrap();

    let rect = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_viewport_viewbox() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="400" viewBox="0 0 100 400">
  <rect id="one" x="0" y="0" width="100" height="200" fill="rgb(0,255,0)"/>
  <rect id="two" x="0" y="200" width="100" height="200" fill="rgb(0,0,255)"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 400.0,
    };

    let (ink_r, logical_r) = renderer
        .geometry_for_layer(Some("#two"), &viewport)
        .unwrap();

    let rect = cairo::Rectangle {
        x: 0.0,
        y: 200.0,
        width: 100.0,
        height: 200.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_for_nonexistent_element() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%"/>
"#,
    );

    let viewport = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    let renderer = CairoRenderer::new(&svg);

    match renderer.geometry_for_layer(Some("#foo"), &viewport) {
        Err(RenderingError::InvalidId(DefsLookupErrorKind::NotFound)) => (),
        _ => panic!(),
    }
}

#[test]
fn layer_geometry_for_invalid_id() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%"/>
"#,
    );

    let viewport = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    let renderer = CairoRenderer::new(&svg);
    match renderer.geometry_for_layer(Some("foo"), &viewport) {
        Err(RenderingError::InvalidId(DefsLookupErrorKind::CannotLookupExternalReferences)) => (),
        _ => panic!(),
    }

    match renderer.geometry_for_layer(Some("foo.svg#foo"), &viewport) {
        Err(RenderingError::InvalidId(DefsLookupErrorKind::CannotLookupExternalReferences)) => (),
        _ => panic!(),
    }

    match renderer.geometry_for_layer(Some(""), &viewport) {
        Err(RenderingError::InvalidId(DefsLookupErrorKind::HrefError(HrefError::ParseError))) => (),
        _ => panic!(),
    }
}

#[test]
fn render_to_viewport_with_different_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    );

    let output_surf = render_document(
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

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 128, 128).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.scale(128.0 / 48.0, 128.0 / 48.0);

        cr.rectangle(8.0, 8.0, 32.0, 32.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "render_to_viewport_with_different_size",
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

    let output_surf = render_document(
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

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(10.0, 20.0);

        cr.rectangle(8.0, 8.0, 32.0, 32.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "render_to_offseted_viewport");
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

    let output_surf = render_document(
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

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(10.0, 20.0);
        cr.translate(-10.0, -10.0);

        cr.rectangle(18.0, 18.0, 32.0, 32.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "render_to_viewport_with_transform",
    );
}

#[test]
fn clip_on_transformed_viewport() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <clipPath id="one" clipPathUnits="objectBoundingBox">
      <path d="M 0.5 0.0 L 1.0 0.5 L 0.5 1.0 L 0.0 0.5 Z"/>
    </clipPath>
  </defs>
  <g clip-path="url(#one)">
    <rect x="10" y="10" width="40" height="40" fill="blue"/>
    <rect x="50" y="50" width="40" height="40" fill="#00ff00"/>
  </g>
</svg>
"##,
    );

    let output_surf = render_document(
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

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(50.0, 50.0);

        cr.push_group();

        cr.rectangle(10.0, 10.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();

        cr.rectangle(50.0, 50.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill();

        cr.pop_group_to_source();

        cr.move_to(50.0, 10.0);
        cr.line_to(90.0, 50.0);
        cr.line_to(50.0, 90.0);
        cr.line_to(10.0, 50.0);
        cr.close_path();

        cr.clip();
        cr.paint();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "clip_on_transformed_viewport",
    );
}

#[test]
fn mask_on_transformed_viewport() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <mask id="one" maskContentUnits="objectBoundingBox">
      <path d="M 0.5 0.0 L 1.0 0.5 L 0.5 1.0 L 0.0 0.5 Z" fill="white"/>
    </mask>
  </defs>
  <g mask="url(#one)">
    <rect x="10" y="10" width="40" height="40" fill="blue"/>
    <rect x="50" y="50" width="40" height="40" fill="#00ff00"/>
  </g>
</svg>
"##,
    );

    let output_surf = render_document(
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

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(50.0, 50.0);

        cr.push_group();

        cr.rectangle(10.0, 10.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();

        cr.rectangle(50.0, 50.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill();

        cr.pop_group_to_source();

        cr.move_to(50.0, 10.0);
        cr.line_to(90.0, 50.0);
        cr.line_to(50.0, 90.0);
        cr.line_to(10.0, 50.0);
        cr.close_path();

        cr.clip();
        cr.paint();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "mask_on_transformed_viewport",
    );
}
