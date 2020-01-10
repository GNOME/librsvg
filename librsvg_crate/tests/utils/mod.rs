#![allow(unused)]
use cairo;
use gio;
use glib;

use librsvg;
use librsvg::{CairoRenderer, Loader, RenderingError, SvgHandle};
use rsvg_internals;

use self::rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use std::env;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

mod compare_surfaces;
use self::compare_surfaces::compare_surfaces;

pub use self::compare_surfaces::BufferDiff;

pub fn load_svg(input: &'static [u8]) -> SvgHandle {
    let bytes = glib::Bytes::from_static(input);
    let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

    Loader::new()
        .read_stream(&stream, None::<&gio::File>, None::<&gio::Cancellable>)
        .unwrap()
}

#[derive(Copy, Clone)]
pub struct SurfaceSize(pub i32, pub i32);

pub fn render_document<F: FnOnce(&cairo::Context)>(
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
        Ok(renderer.render_document(&cr, &viewport)?)
    };

    res.and_then(|_| Ok(SharedImageSurface::wrap(output, SurfaceType::SRgb)?))
}

pub fn output_dir() -> PathBuf {
    let path = PathBuf::from(
        env::var_os("OUT_DIR")
            .expect(r#"OUT_DIR is not set, please set it or run under "cargo test""#),
    );

    fs::create_dir_all(&path).expect("could not create output directory for tests");

    println!("outputting to {}", path.to_string_lossy());

    path
}

pub fn fixture_dir() -> PathBuf {
    let path = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .expect(r#"CARGO_MANIFEST_DIR" is not set, please set it or run under "cargo test""#),
    )
    .join("tests")
    .join("fixtures");

    println!("looking for fixtures at {}", path.to_string_lossy());

    path
}

pub fn compare_to_file(
    output_surf: &SharedImageSurface,
    output_base_name: &str,
    fixture_filename: &str,
) {
    let output_path = output_dir().join(&format!("{}-out.png", output_base_name));
    let fixture_path = fixture_dir().join(fixture_filename);

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
    let expected_surf = SharedImageSurface::wrap(expected, SurfaceType::SRgb).unwrap();

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

const MAX_DIFF: u8 = 2;

pub fn compare_to_surface(
    output_surf: &SharedImageSurface,
    reference_surf: &SharedImageSurface,
    output_base_name: &str,
) {
    let output_path = output_dir().join(&format!("{}-out.png", output_base_name));

    let mut output_file = File::create(output_path).unwrap();
    output_surf
        .clone()
        .into_image_surface()
        .unwrap()
        .write_to_png(&mut output_file)
        .unwrap();

    let diff = compare_surfaces(output_surf, reference_surf).unwrap();

    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            let surf = diff.surface.into_image_surface().unwrap();
            let diff_path = output_dir().join(&format!("{}-diff.png", output_base_name));
            let mut output_file = File::create(diff_path).unwrap();
            surf.write_to_png(&mut output_file).unwrap();

            if diff.num_pixels_changed != 0 && diff.max_diff > MAX_DIFF {
                println!(
                    "{}: {} pixels changed with maximum difference of {}",
                    output_base_name, diff.num_pixels_changed, diff.max_diff,
                );
                unreachable!("surfaces are too different");
            }
        }
    }
}
