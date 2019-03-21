use cairo;
use gio;
use gio::MemoryInputStreamExt;
use glib;
use glib::Cast;

use librsvg;
use librsvg::{CairoRenderer, Loader, RenderingError, SvgHandle};
use rsvg_internals;

use self::rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

mod compare_surfaces;
use self::compare_surfaces::compare_surfaces;

pub use self::compare_surfaces::BufferDiff;

pub fn load_svg(input: &'static [u8]) -> SvgHandle {
    let stream = gio::MemoryInputStream::new();
    stream.add_bytes(&glib::Bytes::from_static(input));

    Loader::new()
        .read_stream(&stream.upcast(), None, None)
        .unwrap()
}

#[derive(Copy, Clone)]
pub struct SurfaceSize(pub i32, pub i32);

pub fn render_to_viewport<F: FnOnce(&cairo::Context)>(
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

pub fn output_dir() -> PathBuf {
    PathBuf::from(
        env::var_os("OUT_DIR")
            .expect(r#"OUT_DIR is not set, please set it or run under "cargo test""#),
    )
}

pub fn test_result(output_surf: &SharedImageSurface, output_base_name: &str, fixture_filename: &str) {
    let output_path = output_dir().join(&format!("{}-out.png", output_base_name));
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
            let diff_path = output_dir().join(&format!("{}-diff.png", output_base_name));
            let mut output_file = File::create(diff_path).unwrap();
            surf.write_to_png(&mut output_file).unwrap();

            assert_eq!(diff.num_pixels_changed, 0);
        }
    }
}
