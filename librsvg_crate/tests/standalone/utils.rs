#![allow(unused)]
use cairo;
use gio;
use glib;

use librsvg::{CairoRenderer, Loader, LoadingError, RenderingError, SvgHandle};

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use rsvg_internals::{compare_surfaces, BufferDiff};

use std::env;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

pub fn load_svg(input: &'static [u8]) -> Result<SvgHandle, LoadingError> {
    let bytes = glib::Bytes::from_static(input);
    let stream = gio::MemoryInputStream::new_from_bytes(&bytes);

    Loader::new().read_stream(&stream, None::<&gio::File>, None::<&gio::Cancellable>)
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
    reference_filename: &str,
) {
    let reference_path = fixture_dir().join(reference_filename);
    let file = File::open(reference_path).unwrap();

    let mut reference_file = BufReader::new(file);

    let reference = cairo::ImageSurface::create_from_png(&mut reference_file).unwrap();
    let reference_surf = SharedImageSurface::wrap(reference, SurfaceType::SRgb).unwrap();

    compare_to_surface(output_surf, &reference_surf, output_base_name);
}

pub fn compare_to_surface(
    output_surf: &SharedImageSurface,
    reference_surf: &SharedImageSurface,
    output_base_name: &str,
) {
    let output_path = output_dir().join(&format!("{}-out.png", output_base_name));

    println!("output:\t{}", output_path.to_string_lossy());

    let mut output_file = File::create(output_path).unwrap();
    output_surf
        .clone()
        .into_image_surface()
        .unwrap()
        .write_to_png(&mut output_file)
        .unwrap();

    let diff = compare_surfaces(output_surf, reference_surf).unwrap();
    evaluate_diff(&diff, output_base_name);
}

const MAX_DIFF: u8 = 2;

fn evaluate_diff(diff: &BufferDiff, output_base_name: &str) {
    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            let surf = diff.surface.clone().into_image_surface().unwrap();
            let diff_path = output_dir().join(&format!("{}-diff.png", output_base_name));
            println!("diff:\t{}", diff_path.to_string_lossy());

            let mut output_file = File::create(diff_path).unwrap();
            surf.write_to_png(&mut output_file).unwrap();

            if diff.num_pixels_changed != 0 && diff.max_diff > MAX_DIFF {
                println!(
                    "{}: {} pixels changed with maximum difference of {}",
                    output_base_name, diff.num_pixels_changed, diff.max_diff,
                );
                panic!("surfaces are too different");
            }
        }
    }
}
