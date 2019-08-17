use cairo;

mod utils;

use self::utils::{load_svg, render_document, SurfaceSize};

// https://gitlab.gnome.org/GNOME/librsvg/issues/496
#[test]
fn inf_width() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg s="Pg" width="1001111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111" heiNht=" 00">
 [l<g mask="url(sHaf:ax-fwiw0\inside\ax-ide\ax-flow#o0" styli="fility:!.5;">>
  </g>
</svg>"#,
    );

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
