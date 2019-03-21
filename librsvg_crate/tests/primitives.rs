use cairo;
use gio;
use glib;
use librsvg;
use rsvg_internals;

mod utils;

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use self::utils::{load_svg, render_to_viewport, compare_to_surface, SurfaceSize};

use std::fs::File;

#[test]
fn simple_opacity_with_transform() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
  <g opacity="0.5">
    <rect x="10" y="10" width="30" height="30" fill="blue"/>
  </g>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
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

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 150, 150).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(50.0, 50.0);
        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::new(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "simple_opacity_with_transform");
}

#[test]
fn simple_opacity_with_offset_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
  <g opacity="0.5">
    <rect x="10" y="10" width="30" height="30" fill="blue"/>
  </g>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(150, 150),
        |_cr| (),
        cairo::Rectangle {
            x: 50.0,
            y: 50.0,
            width: 50.0,
            height: 50.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 150, 150).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(50.0, 50.0);
        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::new(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "simple_opacity_with_offset_viewport");
}

#[test]
fn opacity_inside_transformed_group() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <g transform="translate(20, 20)">
    <rect x="0" y="0" width="60" height="60" style="fill:blue; opacity:0.5;"/>
  </g>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(140, 140),
        |cr| cr.translate(20.0, 20.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 140, 140).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(20.0, 20.0);
        cr.rectangle(20.0, 20.0, 60.0, 60.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill();
    }

    let reference_surf = SharedImageSurface::new(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "opacity_inside_transformed_group");
}

#[test]
fn compound_opacity() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" baseProfile="basic" id="svg-root"
  width="100%" height="100%" viewBox="0 0 480 360"
  xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
  <g>
    <g opacity="0.5">
      <rect x="60" y="230" width="80" height="40" fill="#0000ff" opacity=".5"/>
      <rect x="70" y="240" width="80" height="40" fill="#00ff00" opacity=".5"/>
    </g>
  </g>
</svg>
"##,
    );

    let output_surf = render_to_viewport(
        &svg,
        SurfaceSize(500, 380),
        |cr| cr.translate(10.0, 10.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 480.0,
            height: 360.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 380).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf);

        cr.translate(10.0, 10.0);

        cr.push_group();

        cr.rectangle(60.0, 230.0, 80.0, 40.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill();

        cr.rectangle(70.0, 240.0, 80.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 0.5);
        cr.fill();

        cr.pop_group_to_source();
        cr.paint_with_alpha(0.5);
    }

    let mut file = File::create("/tmp/reference.png").unwrap();
    reference_surf.write_to_png(&mut file).unwrap();

    let reference_surf = SharedImageSurface::new(reference_surf, SurfaceType::SRgb).unwrap();

    compare_to_surface(&output_surf, &reference_surf, "compound_opacity");
}
