use cairo;

use librsvg::{rsvg_convert_only::LegacySize, CairoRenderer};

use crate::utils::load_svg;

#[test]
fn just_viewbox_uses_viewbox_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 200"/>
"#,
    )
        .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).legacy_layer_geometry(None).unwrap(),
        (cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 200.0,
        },
         cairo::Rectangle {
             x: 0.0,
             y: 0.0,
             width: 100.0,
             height: 200.0,
         })
    );
}

#[test]
fn no_intrinsic_size_uses_element_geometries() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
        .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).legacy_layer_geometry(None).unwrap(),
        (cairo::Rectangle {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        },
         cairo::Rectangle {
             x: 10.0,
             y: 20.0,
             width: 30.0,
             height: 40.0,
         })
    );
}

#[test]
fn hundred_percent_width_height_uses_viewbox() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 100 200"/>
"#,
    )
        .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).legacy_layer_geometry(None).unwrap(),
        (cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 200.0,
        },
         cairo::Rectangle {
             x: 0.0,
             y: 0.0,
             width: 100.0,
             height: 200.0,
         })
    );
}

#[test]
fn hundred_percent_width_height_no_viewbox_uses_element_geometries() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
        .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg).legacy_layer_geometry(None).unwrap(),
        (cairo::Rectangle {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        },
         cairo::Rectangle {
             x: 10.0,
             y: 20.0,
             width: 30.0,
             height: 40.0,
         })
    );
}
