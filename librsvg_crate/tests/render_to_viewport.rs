use cairo;
use gio;
use glib;
use librsvg;
use rsvg_internals;

use gio::MemoryInputStreamExt;
use glib::Cast;

use librsvg::{CairoRenderer, Loader, RenderingError, SvgHandle};

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use self::rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

mod compare_surfaces;

use crate::compare_surfaces::{compare_surfaces, BufferDiff};

fn load_svg(input: &'static [u8]) -> SvgHandle {
    let stream = gio::MemoryInputStream::new();
    stream.add_bytes(&glib::Bytes::from_static(input));

    Loader::new()
        .read_stream(&stream.upcast(), None, None)
        .unwrap()
}

#[derive(Copy, Clone)]
struct SurfaceSize(i32, i32);

fn render_to_viewport<F: FnOnce(&cairo::Context)>(
    svg: &SvgHandle,
    surface_size: SurfaceSize,
    cr_transform: F,
    viewport: cairo::Rectangle,
) -> Result<SharedImageSurface, RenderingError> {
    let renderer = CairoRenderer::new(svg);

    let SurfaceSize(width, height) = surface_size;

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();

    let res = {
        let cr = cairo::Context::new(&output);
        cr_transform(&cr);
        Ok(renderer.render_element_to_viewport(&cr, None, &viewport)?)
    };

    res.and_then(|_| Ok(SharedImageSurface::new(output, SurfaceType::SRgb)?))
}

fn test_result(output_surf: &SharedImageSurface, output_base_name: &str, fixture_filename: &str) {
    let output_path = PathBuf::from(&format!("{}-out.png", output_base_name));
    let fixture_path = PathBuf::from(&format!("tests/fixtures/{}", fixture_filename));

    let mut output_file = File::create(output_path).unwrap();
    output_surf
        .clone()
        .into_image_surface()
        .unwrap()
        .write_to_png(&mut output_file)
        .unwrap();

    let file =
        File::open(fixture_path).expect("cannot find {} - are you in the librsvg_crate directory?");

    let mut fixture_file = BufReader::new(file);

    let expected = cairo::ImageSurface::create_from_png(&mut fixture_file).unwrap();
    let expected_surf = SharedImageSurface::new(expected, SurfaceType::SRgb).unwrap();

    let diff = compare_surfaces(output_surf, &expected_surf).unwrap();

    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            let surf = diff.surface.into_image_surface().unwrap();
            let diff_path = PathBuf::from(format!("{}-diff.png", output_base_name));
            let mut output_file = File::create(diff_path).unwrap();
            surf.write_to_png(&mut output_file).unwrap();

            assert_eq!(diff.num_pixels_changed, 0);
        }
    }
}

#[test]
fn render_to_viewport_with_different_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    );

    let output_surf = render_to_viewport(
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

    test_result(
        &output_surf,
        "render_to_viewport_with_different_size",
        "rect-48x48-rendered-128x128.png",
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

    let output_surf = render_to_viewport(
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

    test_result(
        &output_surf,
        "render_to_offseted_viewport",
        "rect-48x48-offsetted-100x100-10x20.png",
    );
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

    let output_surf = render_to_viewport(
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

    test_result(
        &output_surf,
        "render_to_viewport_with_transform",
        "rect-48x48-offsetted-100x100-10x20.png",
    );
}

#[test]
fn clip_on_transformed_viewport() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <clipPath id="one" clipPathUnits="objectBoundingBox">
      <circle cx="0.5" cy="0.5" r="0.5"/>
    </clipPath>
  </defs>
  <g clip-path="url(#one)">
    <rect x="10" y="10" width="40" height="40" fill="blue"/>
    <rect x="50" y="50" width="40" height="40" fill="limegreen"/>
  </g>
</svg>
"##,
    );

    let output_surf = render_to_viewport(
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

    test_result(
        &output_surf,
        "clip_on_transformed_viewport",
        "clip-on-transformed-viewport-200x200.png",
    );
}

#[test]
fn mask_on_transformed_viewport() {
    let svg = load_svg(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <mask id="one" maskContentUnits="objectBoundingBox">
      <circle cx="0.5" cy="0.5" r="0.5" fill="white"/>
    </mask>
  </defs>
  <g mask="url(#one)">
    <rect x="10" y="10" width="40" height="40" fill="blue"/>
    <rect x="50" y="50" width="40" height="40" fill="limegreen"/>
  </g>
</svg>
"##,
    );

    let output_surf = render_to_viewport(
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

    test_result(
        &output_surf,
        "mask_on_transformed_viewport",
        "mask-on-transformed-viewport-200x200.png",
    );
}
