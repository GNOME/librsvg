use gio::prelude::*;

use rsvg::tests_only::{SharedImageSurface, SurfaceType};
use rsvg::{CairoRenderer, RenderingError};

use rsvg::test_utils::load_svg;
use rsvg::test_utils::reference_utils::{Compare, Evaluate, Reference};

#[test]
fn has_element_with_id_works() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
  <rect id="foo" x="10" y="10" width="30" height="30"/>
</svg>
"#,
    )
    .unwrap();

    assert!(svg.has_element_with_id("#foo").unwrap());
    assert!(!svg.has_element_with_id("#bar").unwrap());

    assert!(matches!(
        svg.has_element_with_id(""),
        Err(RenderingError::InvalidId(_))
    ));

    assert!(matches!(
        svg.has_element_with_id("not a fragment"),
        Err(RenderingError::InvalidId(_))
    ));

    assert!(matches!(
        svg.has_element_with_id("notfragment#fragment"),
        Err(RenderingError::InvalidId(_))
    ));
}

#[test]
fn render_layer() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect id="foo" x="10" y="10" width="30" height="30" fill="#00ff00"/>
  <rect id="bar" x="20" y="20" width="30" height="30" fill="#0000ff"/>
</svg>
"##,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    let res = {
        let cr = cairo::Context::new(&output).expect("Failed to create cairo context");
        let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

        renderer.render_layer(&cr, Some("#bar"), &viewport)
    };

    let output_surf = res
        .map(|_| SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap())
        .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(100.0, 100.0);

        cr.rectangle(20.0, 20.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "render_layer");
}

#[test]
fn untransformed_element() {
    // This has a rectangle inside a transformed group.  The rectangle
    // inherits its stroke-width from the group.
    //
    // The idea is that we'll be able to extract the geometry of the rectangle
    // as if it were not transformed by its ancestors, but still retain the
    // cascade from the ancestors.
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <g transform="rotate(45)" stroke-width="10" stroke="#000000">
    <rect id="foo" x="10" y="20" width="30" height="40" fill="#0000ff"/>
  </g>
</svg>
"##,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    /* Measuring */

    let (ink_r, logical_r) = renderer.geometry_for_element(Some("#foo")).unwrap();

    assert_eq!(ink_r, cairo::Rectangle::new(0.0, 0.0, 40.0, 50.0));

    assert_eq!(logical_r, cairo::Rectangle::new(5.0, 5.0, 30.0, 40.0));

    /* Rendering */

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    let res = {
        let cr = cairo::Context::new(&output).expect("Failed to create cairo context");
        let viewport = cairo::Rectangle::new(100.0, 100.0, 100.0, 100.0);

        renderer.render_element(&cr, Some("#foo"), &viewport)
    };

    let output_surf = res
        .map(|_| SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap())
        .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(100.0, 100.0);

        cr.rectangle(10.0, 10.0, 60.0, 80.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill_preserve().unwrap();

        cr.set_line_width(20.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr.stroke().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "untransformed_element");
}

#[test]
fn set_stylesheet() {
    // This has a rectangle which we style from a user-supplied stylesheet.
    let mut svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect id="foo" x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"##,
    )
    .unwrap();

    svg.set_stylesheet("rect { fill: #00ff00; }")
        .expect("should be a valid stylesheet");

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    let res = {
        let cr = cairo::Context::new(&output).expect("Failed to create cairo context");
        let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

        renderer.render_document(&cr, &viewport)
    };

    let output_surf = res
        .map(|_| SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap())
        .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.rectangle(10.0, 20.0, 30.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "set_stylesheet");
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/799
#[test]
fn text_doesnt_leave_points_in_current_path() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <text>Hello world!</text>
</svg>
"##,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
    let cr = cairo::Context::new(&output).unwrap();

    assert!(!cr.has_current_point().unwrap());

    let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

    renderer.render_document(&cr, &viewport).unwrap();

    assert!(!cr.has_current_point().unwrap());
}

#[test]
fn cancellation_works() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect x="0" y="0" width="100%" height="100%" fill="blue"/>
</svg>
"##,
    )
    .unwrap();

    // To test cancellation, we'll start out by creating a cancellable and a renderer, and
    // immediately cancelling the operation.  Then we'll start rendering.  In theory this
    // will cause nothing to be rendered.

    let cancellable = gio::Cancellable::new();

    let renderer = CairoRenderer::new(&svg).with_cancellable(&cancellable);
    cancellable.cancel();

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&output).unwrap();
        let viewport = cairo::Rectangle::new(0.0, 0.0, 100.0, 100.0);

        // Test that cancellation happens...
        assert!(matches!(
            renderer.render_document(&cr, &viewport),
            Err(RenderingError::Cancelled)
        ));
    }

    let output_surf = SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap();

    // ... and test that we got an empty surface, since hopefully cancellation occurred
    // before actually rendering anything.
    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "cancellation_works");
}
