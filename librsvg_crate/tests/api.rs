use cairo;
use librsvg::{CairoRenderer, DefsLookupErrorKind, HrefError, RenderingError};

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

mod utils;
use self::utils::{compare_to_surface, load_svg};

#[test]
fn has_element_with_id_works() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
  <rect id="foo" x="10" y="10" width="30" height="30"/>
</svg>
"#,
    );

    assert!(svg.has_element_with_id("#foo").unwrap());
    assert!(!svg.has_element_with_id("#bar").unwrap());

    assert_eq!(
        svg.has_element_with_id(""),
        Err(RenderingError::InvalidId(DefsLookupErrorKind::HrefError(
            HrefError::ParseError
        )))
    );

    assert_eq!(
        svg.has_element_with_id("not a fragment"),
        Err(RenderingError::InvalidId(
            DefsLookupErrorKind::CannotLookupExternalReferences
        ))
    );

    assert_eq!(
        svg.has_element_with_id("notfragment#fragment"),
        Err(RenderingError::InvalidId(
            DefsLookupErrorKind::CannotLookupExternalReferences
        ))
    );
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
    );

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    let res = {
        let cr = cairo::Context::new(&output);
        let viewport = cairo::Rectangle {
            x: 100.0,
            y: 100.0,
            width: 100.0,
            height: 100.0,
        };

        renderer.render_layer(&cr, Some("#bar"), &viewport)
    };

    let output_surf = res
        .and_then(|_| Ok(SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap()))
        .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(100.0, 100.0);

        cr.rectangle(20.0, 20.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "render_layer");
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
    );

    let renderer = CairoRenderer::new(&svg);

    /* Measuring */

    let (ink_r, logical_r) = renderer.geometry_for_element(Some("#foo")).unwrap();

    assert_eq!(
        ink_r,
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 50.0,
        }
    );

    assert_eq!(
        logical_r,
        cairo::Rectangle {
            x: 5.0,
            y: 5.0,
            width: 30.0,
            height: 40.0,
        }
    );

    /* Rendering */

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    let res = {
        let cr = cairo::Context::new(&output);
        let viewport = cairo::Rectangle {
            x: 100.0,
            y: 100.0,
            width: 100.0,
            height: 100.0,
        };

        renderer.render_element(&cr, Some("#foo"), &viewport)
    };

    let output_surf = res
        .and_then(|_| Ok(SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap()))
        .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 300, 300).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(100.0, 100.0);

        cr.rectangle(10.0, 10.0, 60.0, 80.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill_preserve();

        cr.set_line_width(20.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr.stroke();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "untransformed_element");
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
    );

    svg.set_stylesheet("rect { fill: #00ff00; }")
        .expect("should be a valid stylesheet");

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    let res = {
        let cr = cairo::Context::new(&output);
        let viewport = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        renderer.render_document(&cr, &viewport)
    };

    let output_surf = res
        .and_then(|_| Ok(SharedImageSurface::wrap(output, SurfaceType::SRgb).unwrap()))
        .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.rectangle(10.0, 20.0, 30.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "set_stylesheet");
}
