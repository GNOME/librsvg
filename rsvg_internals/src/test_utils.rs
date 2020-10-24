use crate::surface_utils::compare_surfaces::{compare_surfaces, BufferDiff, Diff};
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use std::convert::TryFrom;
use std::env;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Once;

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

// FIXME: proper errors?
fn load_png_as_argb(path: &PathBuf) -> Result<cairo::ImageSurface, ()> {
    let file = File::open(path).map_err(|_| ())?;

    let mut reference_file = BufReader::new(file);

    let png = cairo::ImageSurface::create_from_png(&mut reference_file).map_err(|_| ())?;
    let argb =
        cairo::ImageSurface::create(cairo::Format::ARgb32, png.get_width(), png.get_height())
            .map_err(|_| ())?;

    {
        // convert to ARGB; the PNG may come as Rgb24
        let cr = cairo::Context::new(&argb);
        cr.set_source_surface(&png, 0.0, 0.0);
        cr.paint();
    }

    Ok(argb)
}

pub fn compare_to_file(
    output_surf: &SharedImageSurface,
    output_base_name: &str,
    reference_path: &PathBuf,
) {
    let png = load_png_as_argb(reference_path).unwrap();
    let reference_surf = SharedImageSurface::wrap(png, SurfaceType::SRgb).unwrap();

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
