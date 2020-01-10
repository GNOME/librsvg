use cairo;

mod utils;

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use self::utils::{compare_to_surface, load_svg, render_document, SurfaceSize};

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
