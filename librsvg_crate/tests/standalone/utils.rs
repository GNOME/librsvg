#![allow(unused)]
use cairo;
use gio;
use glib;

use librsvg::{CairoRenderer, Loader, LoadingError, RenderingError, SvgHandle};

use rsvg_internals::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use rsvg_internals::{compare_surfaces, BufferDiff, Diff};

use std::convert::TryFrom;
use std::env;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Once;

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

fn tolerable_difference() -> u8 {
    static mut TOLERANCE: u8 = 2;

    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        if let Ok(str) = env::var("RSVG_TEST_TOLERANCE") {
            let value: usize = str
                .parse()
                .expect("Can not parse RSVG_TEST_TOLERANCE as a number");
            TOLERANCE =
                u8::try_from(value).expect("RSVG_TEST_TOLERANCE should be between 0 and 255");
        }
    });

    unsafe { TOLERANCE }
}

trait Deviation {
    fn distinguishable(&self) -> bool;
    fn inacceptable(&self) -> bool;
}

impl Deviation for Diff {
    fn distinguishable(&self) -> bool {
        self.max_diff > 2
    }

    fn inacceptable(&self) -> bool {
        self.max_diff > tolerable_difference()
    }
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

    println!("output: {}", output_path.to_string_lossy());

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

fn evaluate_diff(diff: &BufferDiff, output_base_name: &str) {
    match diff {
        BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

        BufferDiff::Diff(diff) => {
            if diff.distinguishable() {
                println!(
                    "{}: {} pixels changed with maximum difference of {}",
                    output_base_name, diff.num_pixels_changed, diff.max_diff,
                );
                save_diff(diff, output_base_name);

                if diff.inacceptable() {
                    panic!("surfaces are too different");
                }
            }
        }
    }

    fn save_diff(diff: &Diff, output_base_name: &str) {
        let surf = diff.surface.clone().into_image_surface().unwrap();
        let diff_path = output_dir().join(&format!("{}-diff.png", output_base_name));
        println!("diff: {}", diff_path.to_string_lossy());

        let mut output_file = File::create(diff_path).unwrap();
        surf.write_to_png(&mut output_file).unwrap();
    }
}
