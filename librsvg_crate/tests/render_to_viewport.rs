use cairo;
use gio;
use glib;
use librsvg;
use rsvg_internals;

use gio::MemoryInputStreamExt;
use glib::Cast;

use librsvg::{CairoRenderer, Loader, SvgHandle};

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

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

#[test]
fn render_to_viewport_with_different_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
  <rect x="8" y="8" width="32" height="32" fill="blue"/>
</svg>
"#,
    );

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 128, 128).unwrap();

    {
        let cr = cairo::Context::new(&output);
        renderer
            .render_element_to_viewport(
                &cr,
                None,
                &cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 128.0,
                    height: 128.0,
                },
            )
            .unwrap();
    }

    let output_surf = SharedImageSurface::new(output, SurfaceType::SRgb).unwrap();

    let fixture_path = Path::new("tests/fixtures/rect-48x48-rendered-128x128.png");
    let mut fixture_file = BufReader::new(File::open(fixture_path).unwrap());

    let expected = cairo::ImageSurface::create_from_png(&mut fixture_file).unwrap();
    let expected_surf = SharedImageSurface::new(expected, SurfaceType::SRgb).unwrap();

    let diff = compare_surfaces(&output_surf, &expected_surf).unwrap();

    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            assert_eq!(diff.num_pixels_changed, 0);
        }
    }
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

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&output);
        renderer
            .render_element_to_viewport(
                &cr,
                None,
                &cairo::Rectangle {
                    x: 10.0,
                    y: 20.0,
                    width: 48.0,
                    height: 48.0,
                },
            )
            .unwrap();
    }

    let mut output_file = File::create(Path::new("output.png")).unwrap();
    output.write_to_png(&mut output_file).unwrap();

    let output_surf = SharedImageSurface::new(output, SurfaceType::SRgb).unwrap();

    let fixture_path = Path::new("tests/fixtures/rect-48x48-offsetted-100x100-10x20.png");
    let mut fixture_file = BufReader::new(File::open(fixture_path).unwrap());

    let expected = cairo::ImageSurface::create_from_png(&mut fixture_file).unwrap();
    let expected_surf = SharedImageSurface::new(expected, SurfaceType::SRgb).unwrap();

    let diff = compare_surfaces(&output_surf, &expected_surf).unwrap();

    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            let surf = diff.surface.into_image_surface().unwrap();
            let mut output_file = File::create(Path::new("diff.png")).unwrap();
            surf.write_to_png(&mut output_file).unwrap();

            assert_eq!(diff.num_pixels_changed, 0);
            
        }
    }
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

    let renderer = CairoRenderer::new(&svg);

    let output = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();

    {
        let cr = cairo::Context::new(&output);
        cr.translate(10.0, 20.0);
        renderer
            .render_element_to_viewport(
                &cr,
                None,
                &cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 48.0,
                    height: 48.0,
                },
            )
            .unwrap();
    }

    let mut output_file = File::create(Path::new("output.png")).unwrap();
    output.write_to_png(&mut output_file).unwrap();

    let output_surf = SharedImageSurface::new(output, SurfaceType::SRgb).unwrap();

    let fixture_path = Path::new("tests/fixtures/rect-48x48-offsetted-100x100-10x20.png");
    let mut fixture_file = BufReader::new(File::open(fixture_path).unwrap());

    let expected = cairo::ImageSurface::create_from_png(&mut fixture_file).unwrap();
    let expected_surf = SharedImageSurface::new(expected, SurfaceType::SRgb).unwrap();

    let diff = compare_surfaces(&output_surf, &expected_surf).unwrap();

    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            let surf = diff.surface.into_image_surface().unwrap();
            let mut output_file = File::create(Path::new("diff.png")).unwrap();
            surf.write_to_png(&mut output_file).unwrap();

            assert_eq!(diff.num_pixels_changed, 0);
            
        }
    }
}
