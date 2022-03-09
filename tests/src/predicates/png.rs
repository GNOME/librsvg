use png;
use predicates::prelude::*;
use predicates::reflection::{Case, Child, PredicateReflection, Product};
use std::fmt;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use librsvg::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use crate::compare_surfaces::BufferDiff;
use crate::reference_utils::{surface_from_png, Compare, Deviation, Reference};

/// Checks that the variable of type [u8] can be parsed as a PNG file.
#[derive(Debug)]
pub struct PngPredicate {}

impl PngPredicate {
    pub fn with_size(self: Self, w: u32, h: u32) -> SizePredicate<Self> {
        SizePredicate::<Self> { p: self, w, h }
    }

    pub fn with_contents<P: AsRef<Path>>(self: Self, reference: P) -> ReferencePredicate<Self> {
        let mut path = PathBuf::new();
        path.push(reference);
        ReferencePredicate::<Self> { p: self, path }
    }
}

impl Predicate<[u8]> for PngPredicate {
    fn eval(&self, data: &[u8]) -> bool {
        let decoder = png::Decoder::new(data);
        decoder.read_info().is_ok()
    }

    fn find_case<'a>(&'a self, _expected: bool, data: &[u8]) -> Option<Case<'a>> {
        let decoder = png::Decoder::new(data);
        match decoder.read_info() {
            Ok(_) => None,
            Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
        }
    }
}

impl PredicateReflection for PngPredicate {}

impl fmt::Display for PngPredicate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "is a PNG")
    }
}

/// Extends a PngPredicate by a check for a given size of the PNG file.
#[derive(Debug)]
pub struct SizePredicate<PngPredicate> {
    p: PngPredicate,
    w: u32,
    h: u32,
}

impl SizePredicate<PngPredicate> {
    fn eval_info(&self, info: &png::Info) -> bool {
        info.width == self.w && info.height == self.h
    }

    fn find_case_for_info<'a>(&'a self, expected: bool, info: &png::Info) -> Option<Case<'a>> {
        if self.eval_info(info) == expected {
            let product = self.product_for_info(info);
            Some(Case::new(Some(self), false).add_product(product))
        } else {
            None
        }
    }

    fn product_for_info(&self, info: &png::Info) -> Product {
        let actual_size = format!("{} x {}", info.width, info.height);
        Product::new("actual size", actual_size)
    }
}

impl Predicate<[u8]> for SizePredicate<PngPredicate> {
    fn eval(&self, data: &[u8]) -> bool {
        let decoder = png::Decoder::new(data);
        match decoder.read_info() {
            Ok(reader) => self.eval_info(&reader.info()),
            _ => false,
        }
    }

    fn find_case<'a>(&'a self, expected: bool, data: &[u8]) -> Option<Case<'a>> {
        let decoder = png::Decoder::new(data);
        match decoder.read_info() {
            Ok(reader) => self.find_case_for_info(expected, reader.info()),
            Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
        }
    }
}

impl PredicateReflection for SizePredicate<PngPredicate> {
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = Child<'a>> + 'a> {
        let params = vec![Child::new("predicate", &self.p)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for SizePredicate<PngPredicate> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "is a PNG with size {} x {}", self.w, self.h)
    }
}

/// Extends a PngPredicate by a comparison to the contents of a reference file
#[derive(Debug)]
pub struct ReferencePredicate<PngPredicate> {
    p: PngPredicate,
    path: PathBuf,
}

impl ReferencePredicate<PngPredicate> {
    fn diff_acceptable(diff: &BufferDiff) -> bool {
        match diff {
            BufferDiff::DifferentSizes => false,
            BufferDiff::Diff(diff) => !diff.inacceptable(),
        }
    }

    fn diff_surface(&self, surface: &SharedImageSurface) -> Option<BufferDiff> {
        let reference = Reference::from_png(&self.path)
            .unwrap_or_else(|_| panic!("could not open {:?}", self.path));
        if let Ok(diff) = reference.compare(&surface) {
            if !Self::diff_acceptable(&diff) {
                return Some(diff);
            }
        }
        None
    }

    fn find_case_for_surface<'a>(
        &'a self,
        expected: bool,
        surface: &SharedImageSurface,
    ) -> Option<Case<'a>> {
        let diff = self.diff_surface(&surface);
        if diff.is_some() != expected {
            let product = self.product_for_diff(&diff.unwrap());
            Some(Case::new(Some(self), false).add_product(product))
        } else {
            None
        }
    }

    fn product_for_diff(&self, diff: &BufferDiff) -> Product {
        let difference = format!("{}", diff);
        Product::new("images differ", difference)
    }
}

impl Predicate<[u8]> for ReferencePredicate<PngPredicate> {
    fn eval(&self, data: &[u8]) -> bool {
        if let Ok(surface) = surface_from_png(&mut BufReader::new(data)) {
            let surface = SharedImageSurface::wrap(surface, SurfaceType::SRgb).unwrap();
            self.diff_surface(&surface).is_some()
        } else {
            false
        }
    }

    fn find_case<'a>(&'a self, expected: bool, data: &[u8]) -> Option<Case<'a>> {
        match surface_from_png(&mut BufReader::new(data)) {
            Ok(surface) => {
                let surface = SharedImageSurface::wrap(surface, SurfaceType::SRgb).unwrap();
                self.find_case_for_surface(expected, &surface)
            }
            Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
        }
    }
}

impl PredicateReflection for ReferencePredicate<PngPredicate> {
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = Child<'a>> + 'a> {
        let params = vec![Child::new("predicate", &self.p)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for ReferencePredicate<PngPredicate> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "is a PNG that matches the reference {}",
            self.path.display()
        )
    }
}
