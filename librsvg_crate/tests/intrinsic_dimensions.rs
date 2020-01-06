use cairo;

use librsvg::{
    CairoRenderer, DefsLookupErrorKind, HrefError, IntrinsicDimensions, Length, LengthUnit,
    RenderingError,
};

mod utils;

use self::utils::load_svg;

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
