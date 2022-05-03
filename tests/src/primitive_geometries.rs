use cairo;
use librsvg::CairoRenderer;

use crate::utils::load_svg;

#[test]
fn rect() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect id="a" x="10" y="20" width="30" height="40"/>
</svg>
"#,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg).test_mode(true);

    let (ink_rect, logical_rect) = renderer
        .geometry_for_layer(
            Some("#a"),
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        )
        .unwrap();

    let expected_ink = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    let expected_logical = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!(ink_rect, expected_ink);
    assert_eq!(logical_rect, expected_logical);
}

#[test]
fn rect_stroke() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect id="a" x="10" y="20" width="30" height="40" stroke-width="10" stroke="black"/>
</svg>
"#,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg).test_mode(true);

    let (ink_rect, logical_rect) = renderer
        .geometry_for_layer(
            Some("#a"),
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        )
        .unwrap();

    let expected_ink = cairo::Rectangle {
        x: 5.0,
        y: 15.0,
        width: 40.0,
        height: 50.0,
    };

    let expected_logical = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!(ink_rect, expected_ink);
    assert_eq!(logical_rect, expected_logical);
}

#[test]
fn rect_stroke_unfilled() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect id="a" x="10" y="20" width="30" height="40" stroke-width="10" stroke="black" fill="none"/>
</svg>
"#,
    )
    .unwrap();

    let renderer = CairoRenderer::new(&svg).test_mode(true);

    let (ink_rect, logical_rect) = renderer
        .geometry_for_layer(
            Some("#a"),
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        )
        .unwrap();

    let expected_ink = cairo::Rectangle {
        x: 5.0,
        y: 15.0,
        width: 40.0,
        height: 50.0,
    };

    let expected_logical = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!(ink_rect, expected_ink);
    assert_eq!(logical_rect, expected_logical);
}
