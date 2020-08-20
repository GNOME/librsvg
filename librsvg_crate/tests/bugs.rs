use cairo;

mod utils;

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use self::utils::{compare_to_surface, load_svg, render_document, SurfaceSize};

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

// https://gitlab.gnome.org/GNOME/librsvg/issues/547
#[test]
fn nonexistent_image_shouldnt_cancel_rendering() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
     width="50" height="50">
  <image xlink:href="nonexistent.png" width="10" height="10"/>
  <rect x="10" y="10" width="30" height="30" fill="blue"/>
</svg>
"#,
    );

    let output_surf = render_document(
        &svg,
        SurfaceSize(50, 50),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 50, 50).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "nonexistent_image_shouldnt_cancel_rendering",
    );
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/568
#[test]
fn href_attribute_overrides_xlink_href() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:xlink="http://www.w3.org/1999/xlink"
     width="500" height="500">
  <defs>
    <rect id="one" x="100" y="100" width="100" height="100" fill="red"/>
    <rect id="two" x="100" y="100" width="100" height="100" fill="lime"/>
  </defs>

  <!-- Per https://svgwg.org/svg2-draft/linking.html#XLinkRefAttrs a plain
       href attribute overrides an xlink:href one in SVG2 -->
  <use xlink:href="#one" href="#two"/>
</svg>
"##,
    );

    let output_surf = render_document(
        &svg,
        SurfaceSize(500, 500),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 500.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.rectangle(100.0, 100.0, 100.0, 100.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "href_attribute_overrides_xlink_href",
    );
}

// https://gitlab.gnome.org/GNOME/librsvg/-/issues/560
#[test]
fn nonexistent_filter_leaves_object_unfiltered() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     width="500" height="500">
  <rect x="100" y="100" width="100" height="100" fill="lime" filter="url(#nonexistent)"/>
</svg>
"##,
    );

    let output_surf = render_document(
        &svg,
        SurfaceSize(500, 500),
        |_| (),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 500.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.rectangle(100.0, 100.0, 100.0, 100.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::wrap(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(
        &output_surf,
        &reference_surf,
        "nonexistent_filter_leaves_object_unfiltered",
    );
}
