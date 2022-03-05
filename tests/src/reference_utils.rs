//! Utilities for the reference image test suite.
//!
//! This module has utility functions that are used in the test suite
//! to compare rendered surfaces to reference images.

use cairo;

use std::convert::TryFrom;
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Once;

use librsvg::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use crate::compare_surfaces::{compare_surfaces, BufferDiff, Diff};

pub struct Reference(SharedImageSurface);

impl Reference {
    pub fn from_png<P>(path: P) -> Result<Self, cairo::IoError>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path).map_err(|e| cairo::IoError::Io(e))?;
        let mut reader = BufReader::new(file);
        let surface = surface_from_png(&mut reader)?;
        Self::from_surface(surface)
    }

    pub fn from_surface(surface: cairo::ImageSurface) -> Result<Self, cairo::IoError> {
        let shared = SharedImageSurface::wrap(surface, SurfaceType::SRgb)?;
        Ok(Self(shared))
    }
}

pub trait Compare {
    fn compare(self, surface: &SharedImageSurface) -> Result<BufferDiff, cairo::IoError>;
}

impl Compare for &Reference {
    fn compare(self, surface: &SharedImageSurface) -> Result<BufferDiff, cairo::IoError> {
        compare_surfaces(&self.0, surface).map_err(cairo::IoError::from)
    }
}

impl Compare for Result<Reference, cairo::IoError> {
    fn compare(self, surface: &SharedImageSurface) -> Result<BufferDiff, cairo::IoError> {
        self.map(|reference| reference.compare(surface))
            .and_then(std::convert::identity)
    }
}

pub trait Evaluate {
    fn evaluate(&self, output_surface: &SharedImageSurface, output_base_name: &str);
}

impl Evaluate for BufferDiff {
    /// Evaluates a BufferDiff and panics if there are relevant differences
    ///
    /// The `output_base_name` is used to write test results if the
    /// surfaces are different.  If this is `foo`, this will write
    /// `foo-out.png` with the `output_surf` and `foo-diff.png` with a
    /// visual diff between `output_surf` and the `Reference` that this
    /// diff was created from.
    ///
    /// # Panics
    ///
    /// Will panic if the surfaces are too different to be acceptable.
    fn evaluate(&self, output_surf: &SharedImageSurface, output_base_name: &str) {
        match self {
            BufferDiff::DifferentSizes => unreachable!("surfaces should be of the same size"),

            BufferDiff::Diff(diff) => {
                if diff.distinguishable() {
                    println!(
                        "{}: {} pixels changed with maximum difference of {}",
                        output_base_name, diff.num_pixels_changed, diff.max_diff,
                    );

                    write_to_file(output_surf, output_base_name, "out");
                    write_to_file(&diff.surface, output_base_name, "diff");

                    if diff.inacceptable() {
                        panic!("surfaces are too different");
                    }
                }
            }
        }
    }
}

impl Evaluate for Result<BufferDiff, cairo::IoError> {
    fn evaluate(&self, output_surface: &SharedImageSurface, output_base_name: &str) {
        self.as_ref()
            .map(|diff| diff.evaluate(output_surface, output_base_name))
            .unwrap();
    }
}

fn write_to_file(input: &SharedImageSurface, output_base_name: &str, suffix: &str) {
    let path = output_dir().join(&format!("{}-{}.png", output_base_name, suffix));
    println!("{}: {}", suffix, path.to_string_lossy());
    let mut output_file = File::create(path).unwrap();
    input
        .clone()
        .into_image_surface()
        .unwrap()
        .write_to_png(&mut output_file)
        .unwrap();
}

/// Creates a directory for test output and returns its path.
///
/// The location for the output directory is taken from the `OUT_DIR` environment
/// variable if that is set. Otherwise std::env::temp_dir() will be used, which is
/// a platform dependent location for temporary files.
///
/// # Panics
///
/// Will panic if the output directory can not be created.
pub fn output_dir() -> PathBuf {
    let tempdir = || {
        let mut path = env::temp_dir();
        path.push("rsvg-test-output");
        path
    };
    let path = env::var_os("OUT_DIR").map_or_else(tempdir, PathBuf::from);

    fs::create_dir_all(&path).expect("could not create output directory for tests");

    path
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

pub trait Deviation {
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

/// Creates a cairo::ImageSurface from a stream of PNG data.
///
/// The surface is converted to ARGB if needed. Use this helper function with `Reference`.
pub fn surface_from_png<R>(stream: &mut R) -> Result<cairo::ImageSurface, cairo::IoError>
where
    R: Read,
{
    let png = cairo::ImageSurface::create_from_png(stream)?;
    let argb = cairo::ImageSurface::create(cairo::Format::ARgb32, png.width(), png.height())?;
    {
        // convert to ARGB; the PNG may come as Rgb24
        let cr = cairo::Context::new(&argb).expect("Failed to create a cairo context");
        cr.set_source_surface(&png, 0.0, 0.0).unwrap();
        cr.paint().unwrap();
    }
    Ok(argb)
}

/// Macro test that compares render outputs
///
/// Takes in SurfaceSize width and height, setting the cairo surface
#[macro_export]
macro_rules! test_compare_render_output {
    ($test_name:ident, $width:expr, $height:expr, $test:expr, $reference:expr $(,)?) => {
        #[test]
        fn $test_name() {
            crate::utils::setup_font_map();

            let sx: i32 = $width;
            let sy: i32 = $height;
            let svg = load_svg($test).unwrap();
            let output_surf = render_document(
                &svg,
                SurfaceSize(sx, sy),
                |_| (),
                cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: sx as f64,
                    height: sy as f64,
                },
            )
            .unwrap();

            let reference = load_svg($reference).unwrap();
            let reference_surf = render_document(
                &reference,
                SurfaceSize(sx, sy),
                |_| (),
                cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: sx as f64,
                    height: sy as f64,
                },
            )
            .unwrap();

            Reference::from_surface(reference_surf.into_image_surface().unwrap())
                .compare(&output_surf)
                .evaluate(&output_surf, stringify!($test_name));
        }
    };
}

/// Render two SVG files and compare them.
///
/// This is used to implement reference tests, or reftests.  Use it like this:
///
/// ```ignore
/// test_svg_reference!(test_name, "tests/fixtures/blah/foo.svg", "tests/fixtures/blah/foo-ref.svg");
/// ```
///
/// This will ensure that `foo.svg` and `foo-ref.svg` have exactly the same intrinsic dimensions,
/// and that they produce the same rendered output.
#[macro_export]
macro_rules! test_svg_reference {
    ($test_name:ident, $test_filename:expr, $reference_filename:expr) => {
        #[test]
        fn $test_name() {
            use crate::reference_utils::{Compare, Evaluate, Reference};
            use crate::utils::{render_document, setup_font_map, SurfaceSize};
            use cairo;
            use librsvg::{CairoRenderer, Loader};

            setup_font_map();

            let svg = Loader::new()
                .read_path($test_filename)
                .expect("reading SVG test file");
            let reference = Loader::new()
                .read_path($reference_filename)
                .expect("reading reference file");

            let svg_renderer = CairoRenderer::new(&svg);
            let ref_renderer = CairoRenderer::new(&reference);

            let svg_dim = svg_renderer.intrinsic_dimensions();
            let ref_dim = ref_renderer.intrinsic_dimensions();

            assert_eq!(
                svg_dim, ref_dim,
                "sizes of SVG document and reference file are different"
            );

            let pixels = svg_renderer
                .intrinsic_size_in_pixels()
                .unwrap_or((100.0, 100.0));

            let output_surf = render_document(
                &svg,
                SurfaceSize(pixels.0.ceil() as i32, pixels.1.ceil() as i32),
                |_| (),
                cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: pixels.0,
                    height: pixels.1,
                },
            )
            .unwrap();

            let reference_surf = render_document(
                &reference,
                SurfaceSize(pixels.0.ceil() as i32, pixels.1.ceil() as i32),
                |_| (),
                cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: pixels.0,
                    height: pixels.1,
                },
            )
            .unwrap();

            Reference::from_surface(reference_surf.into_image_surface().unwrap())
                .compare(&output_surf)
                .evaluate(&output_surf, stringify!($test_name));
        }
    };
}
