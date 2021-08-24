use cairo;

use crate::reference_utils::{Compare, Evaluate, Reference};
use crate::test_compare_render_output;
use crate::utils::{load_svg, render_document, SurfaceSize};

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
    )
    .unwrap();

    let output_surf = render_document(
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
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(50.0, 50.0);
        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "simple_opacity_with_transform");
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
    )
    .unwrap();

    let output_surf = render_document(
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
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(50.0, 50.0);
        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "simple_opacity_with_offset_viewport");
}

#[test]
// https://gitlab.gnome.org/GNOME/librsvg/issues/471
fn simple_opacity_with_scale() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
  <g opacity="0.5">
    <rect x="10" y="10" width="30" height="30" fill="blue"/>
  </g>
</svg>
"#,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(500, 500),
        |cr| {
            cr.translate(50.0, 50.0);
            cr.scale(8.0, 8.0);
        },
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 500, 500).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(50.0, 50.0);
        cr.scale(8.0, 8.0);
        cr.rectangle(10.0, 10.0, 30.0, 30.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "simple_opacity_with_scale");
}

#[test]
// https://gitlab.gnome.org/GNOME/librsvg/issues/471
fn markers_with_scale() {
    let svg = load_svg(
        br#"<svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <marker id="marker1" refX="10" refY="10" markerWidth="20" markerHeight="20" orient="auto">
        <path id="marker-path" d="M 20 10 L 0 16 V 4 Z" fill="blue" opacity="0.5"/>
    </marker>
    <path d="M 30 100 L 170 100"
          fill="none" stroke="green"
          marker-start="url(#marker1)" marker-end="url(#marker1)"/>
</svg>

"#,
    )
    .unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(800, 800),
        |cr| {
            cr.scale(4.0, 4.0);
        },
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
        },
    )
    .unwrap();

    let reference_surf = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 800).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.scale(4.0, 4.0);

        cr.move_to(30.0, 100.0);
        cr.line_to(170.0, 100.0);
        cr.set_source_rgb(0.0, 0.5, 0.0);
        cr.set_line_width(1.0);
        cr.stroke().unwrap();

        for (x, y) in &[(30.0, 100.0), (170.0, 100.0)] {
            cr.move_to(x + 20.0 - 10.0, y + 10.0 - 10.0);
            cr.line_to(x + 0.0 - 10.0, y + 16.0 - 10.0);
            cr.line_to(x + 0.0 - 10.0, y + 4.0 - 10.0);
            cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
            cr.fill().unwrap();
        }
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "markers_with_scale");
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
    )
    .unwrap();

    let output_surf = render_document(
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
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(20.0, 20.0);
        cr.rectangle(20.0, 20.0, 60.0, 60.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "opacity_inside_transformed_group");
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
    )
    .unwrap();

    let output_surf = render_document(
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
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(10.0, 10.0);

        cr.push_group();

        cr.rectangle(60.0, 230.0, 80.0, 40.0);
        cr.set_source_rgba(0.0, 0.0, 1.0, 0.5);
        cr.fill().unwrap();

        cr.rectangle(70.0, 240.0, 80.0, 40.0);
        cr.set_source_rgba(0.0, 1.0, 0.0, 0.5);
        cr.fill().unwrap();

        cr.pop_group_to_source().unwrap();
        cr.paint_with_alpha(0.5).unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "compound_opacity");
}

#[test]
fn nested_masks() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="321.00" height="27.00" viewBox="0 0 6420 540">
  <defs>
    <mask id="Mask_big_ex_small" maskUnits="userSpaceOnUse" x="0" y="0" width="6420" height="540">
      <g>
	<use xlink:href="#big" fill="white"/>
	<use xlink:href="#small" fill="black"/>
      </g>
    </mask>
    <g id="big_ex_small">
      <use xlink:href="#big" mask="url(#Mask_big_ex_small)"/>
    </g>
    <mask id="Region0" maskUnits="userSpaceOnUse" x="0" y="0" width="6420" height="540" fill-rule="nonzero">
      <use xlink:href="#big_ex_small" fill="white"/>
    </mask>
    <rect id="big" x="0" y="0" width="6420" height="540"/>
    <rect id="small" x="2760" y="20" width="900" height="480"/>
  </defs>
  <g mask="url(#Region0)">
    <g transform="matrix(1.66667 0 0 1.66667 0 0)">
      <rect x="0" y="0" width="6420" height="540" fill="black"/>
    </g>
  </g>
</svg>

"##,
    ).unwrap();

    let output_surf = render_document(
        &svg,
        SurfaceSize(321 + 20, 27 + 20),
        |cr| cr.translate(10.0, 10.0),
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 321.0,
            height: 27.0,
        },
    )
    .unwrap();

    let reference_surf =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 321 + 20, 27 + 20).unwrap();

    {
        let cr = cairo::Context::new(&reference_surf).expect("Failed to create a cairo context");

        cr.translate(10.0, 10.0);
        cr.scale(321.0 / 6420.0, 27.0 / 540.0);

        cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        cr.rectangle(0.0, 0.0, 6420.0, 540.0);
        cr.fill().unwrap();

        cr.set_operator(cairo::Operator::Clear);
        cr.rectangle(2760.0, 20.0, 900.0, 480.0);
        cr.fill().unwrap();
    }

    Reference::from_surface(reference_surf)
        .compare(&output_surf)
        .evaluate(&output_surf, "nested_masks");
}

test_compare_render_output!(
    image_element_with_no_href,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <image x="10" y="10"/>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
    </svg>"##,
);

test_compare_render_output!(
    transform_property,
    100,
    100,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect x="50" y="50" width="20" height="20" fill="black"/>
</svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <rect x="10" y="10" width="20" height="20" fill="black" style="transform: translate(40px, 40px);"/>
</svg>"##,
);

test_compare_render_output!(
    compound_transform_property,
    100,
    100,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="-30 -30 60 60">
  <rect x="0" y="0" width="20" height="20" fill="black" style="transform: translate(-10px, -20px) scale(2) rotate(45deg);"/>
</svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="-30 -30 60 60">
  <rect x="0" y="0" width="20" height="20" fill="black" transform="translate(-10 -20) scale(2) rotate(45)"/>
</svg>"##,
);
