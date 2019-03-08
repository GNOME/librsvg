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
