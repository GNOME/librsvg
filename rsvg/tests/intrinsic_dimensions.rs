use rsvg::{CairoRenderer, IntrinsicDimensions, Length, LengthUnit, RenderingError};

use rsvg::test_utils::reference_utils::{Compare, Evaluate, Reference};
use rsvg::test_utils::{load_svg, render_document, SurfaceSize};

#[test]
fn no_intrinsic_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"/>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).intrinsic_dimensions(),
        IntrinsicDimensions {
            width: Length::new(1.0, LengthUnit::Percent),
            height: Length::new(1.0, LengthUnit::Percent),
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
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).intrinsic_dimensions(),
        IntrinsicDimensions {
            width: Length::new(10.0, LengthUnit::Cm),
            height: Length::new(20.0, LengthUnit::Px),
            vbox: Some(cairo::Rectangle::new(0.0, 0.0, 100.0, 200.0)),
        }
    );
}

#[test]
fn intrinsic_size_in_pixels() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20" viewBox="0 0 100 200"/>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).intrinsic_size_in_pixels(),
        Some((10.0, 20.0)),
    );
}

#[test]
fn no_intrinsic_size_in_pixels_with_percent_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 100 200"/>
"#,
    )
    .unwrap();

    assert_eq!(CairoRenderer::new(&svg).intrinsic_size_in_pixels(), None);
}

#[test]
fn no_intrinsic_size_in_pixels_with_no_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 200"/>
"#,
    )
    .unwrap();

    assert_eq!(CairoRenderer::new(&svg).intrinsic_size_in_pixels(), None);
}

#[test]
fn no_intrinsic_size_in_pixels_with_one_missing_dimension() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" viewBox="0 0 100 200"/>
"#,
    )
    .unwrap();

    assert_eq!(CairoRenderer::new(&svg).intrinsic_size_in_pixels(), None);
}

#[test]
fn root_geometry_with_percent_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect x="10" y="20" width="30" height="40"/>
</svg>
"#,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle::new(10.0, 20.0, 30.0, 40.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle::new(110.0, 120.0, 30.0, 40.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle::new(110.0, 120.0, 30.0, 40.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    let rect = cairo::Rectangle::new(105.0, 110.0, 15.0, 20.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    // The SVG document above has no width/height nor viewBox, which means it should
    // start with an identity transform for its coordinate space.  Since the viewport
    // is just offset by (100, 100), this just translates the coordinates of the <rect>.
    let rect = cairo::Rectangle::new(210.0, 220.0, 50.0, 40.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer.geometry_for_layer(None, &viewport).unwrap();

    // Lack of viewBox means we use an identity transform, so the <rect> is just
    // offset by (100, 100) because of the viewport.
    let rect = cairo::Rectangle::new(110.0, 120.0, 50.0, 40.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    let (ink_r, logical_r) = renderer
        .geometry_for_layer(Some("#foo"), &viewport)
        .unwrap();

    let rect = cairo::Rectangle::new(10.0, 20.0, 30.0, 40.0);

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
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 400.0);

    let (ink_r, logical_r) = renderer
        .geometry_for_layer(Some("#two"), &viewport)
        .unwrap();

    let rect = cairo::Rectangle::new(0.0, 200.0, 100.0, 200.0);

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn layer_geometry_for_nonexistent_element() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%"/>
"#,
    )
    .unwrap();

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    let renderer = CairoRenderer::new(&svg);

    assert!(matches!(
        renderer.geometry_for_layer(Some("#foo"), &viewport),
        Err(RenderingError::IdNotFound)
    ));
}

#[test]
fn layer_geometry_for_invalid_id() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%"/>
"#,
    )
    .unwrap();

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    let renderer = CairoRenderer::new(&svg);
    assert!(matches!(
        renderer.geometry_for_layer(Some("foo"), &viewport),
        Err(RenderingError::InvalidId(_))
    ));

    assert!(matches!(
        renderer.geometry_for_layer(Some("foo.svg#foo"), &viewport),
        Err(RenderingError::InvalidId(_))
    ));

    assert!(matches!(
        renderer.geometry_for_layer(Some(""), &viewport),
        Err(RenderingError::InvalidId(_))
    ));
}

#[test]
fn render_to_viewport_with_different_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(128, 128),
        |_cr| (),
        cairo::Rectangle::new(0.0, 0.0, 128.0, 128.0),
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 128, 128).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.scale(128.0 / 48.0, 128.0 / 48.0);

        cr.rectangle(8.0, 8.0, 32.0, 32.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "render_to_viewport_with_different_size");
}

#[test]
fn render_to_offsetted_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(100, 100),
        |_cr| (),
        cairo::Rectangle::new(10.0, 20.0, 48.0, 48.0),
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(10.0, 20.0);

        cr.rectangle(8.0, 8.0, 32.0, 32.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "render_to_offsetted_viewport");
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
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(100, 100),
        |cr| cr.translate(10.0, 20.0),
        cairo::Rectangle::new(0.0, 0.0, 48.0, 48.0),
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(10.0, 20.0);
        cr.translate(-10.0, -10.0);

        cr.rectangle(18.0, 18.0, 32.0, 32.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "render_to_viewport_with_transform");
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
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(200, 200),
        |cr| cr.translate(50.0, 50.0),
        cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0),
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(50.0, 50.0);

        cr.push_group();

        cr.rectangle(10.0, 10.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();

        cr.rectangle(50.0, 50.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();

        cr.pop_group_to_source().unwrap();

        cr.move_to(50.0, 10.0);
        cr.line_to(90.0, 50.0);
        cr.line_to(50.0, 90.0);
        cr.line_to(10.0, 50.0);
        cr.close_path();

        cr.clip();
        cr.paint().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "clip_on_transformed_viewport");
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
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(200, 200),
        |cr| cr.translate(50.0, 50.0),
        cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0),
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(50.0, 50.0);

        cr.push_group();

        cr.rectangle(10.0, 10.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();

        cr.rectangle(50.0, 50.0, 40.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();

        cr.pop_group_to_source().unwrap();

        cr.move_to(50.0, 10.0);
        cr.line_to(90.0, 50.0);
        cr.line_to(50.0, 90.0);
        cr.line_to(10.0, 50.0);
        cr.close_path();

        cr.clip();
        cr.paint().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "mask_on_transformed_viewport");
}
