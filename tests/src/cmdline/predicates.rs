extern crate png;
extern crate predicates;

pub mod file {

    use predicates::boolean::AndPredicate;
    use predicates::prelude::*;
    use predicates::reflection::{Case, Child, PredicateReflection, Product};
    use predicates::str::{ContainsPredicate, StartsWithPredicate};

    use std::fmt;

    /// Checks that the variable of type [u8] looks like a PDF file.
    /// Actually it only looks at the very first bytes.
    #[derive(Debug)]
    pub struct PdfPredicate {}

    impl PdfPredicate {
        fn not_a_pdf<'a>(&'a self, reason: &'static str) -> Option<Case<'a>> {
            Some(Case::new(Some(self), false).add_product(Product::new("not a PDF", reason)))
        }
    }

    impl Predicate<[u8]> for PdfPredicate {
        fn eval(&self, data: &[u8]) -> bool {
            match data.get(0..5) {
                Some(head) => head == b"%PDF-",
                None => false,
            }
        }

        fn find_case<'a>(&'a self, _expected: bool, data: &[u8]) -> Option<Case<'a>> {
            match data.get(0..5) {
                Some(head) => {
                    if head == b"%PDF-" {
                        None
                    } else {
                        self.not_a_pdf("header mismatch")
                    }
                }
                None => self.not_a_pdf("too short"),
            }
        }
    }

    impl PredicateReflection for PdfPredicate {}

    impl fmt::Display for PdfPredicate {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "has PDF header")
        }
    }

    /// Checks that the variable of type [u8] can be parsed as a PNG file.
    #[derive(Debug)]
    pub struct PngPredicate {}

    impl PngPredicate {
        pub fn with_size(self: Self, w: u32, h: u32) -> SizePredicate<Self> {
            SizePredicate::<Self> { p: self, w, h }
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
        fn eval_info(&self, info: &png::OutputInfo) -> bool {
            info.width == self.w && info.height == self.h
        }

        fn find_case_for_info<'a>(
            &'a self,
            expected: bool,
            info: &png::OutputInfo,
        ) -> Option<Case<'a>> {
            if self.eval_info(info) == expected {
                let product = self.product_for_info(info);
                Some(Case::new(Some(self), false).add_product(product))
            } else {
                None
            }
        }

        fn product_for_info(&self, info: &png::OutputInfo) -> Product {
            let actual_size = format!("{} x {}", info.width, info.height);
            Product::new("actual size", actual_size)
        }
    }

    impl Predicate<[u8]> for SizePredicate<PngPredicate> {
        fn eval(&self, data: &[u8]) -> bool {
            let decoder = png::Decoder::new(data);
            match decoder.read_info() {
                Ok((info, _)) => self.eval_info(&info),
                _ => false,
            }
        }

        fn find_case<'a>(&'a self, expected: bool, data: &[u8]) -> Option<Case<'a>> {
            let decoder = png::Decoder::new(data);
            match decoder.read_info() {
                Ok((info, _)) => self.find_case_for_info(expected, &info),
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

    /// Predicates to check that some output ([u8]) is of a certain file type

    pub fn is_png() -> PngPredicate {
        PngPredicate {}
    }

    pub fn is_ps() -> StartsWithPredicate {
        predicate::str::starts_with("%!PS-Adobe-3.0\n")
    }

    pub fn is_eps() -> StartsWithPredicate {
        predicate::str::starts_with("%!PS-Adobe-3.0 EPSF-3.0\n")
    }

    pub fn is_pdf() -> PdfPredicate {
        PdfPredicate {}
    }

    pub fn is_svg() -> AndPredicate<StartsWithPredicate, ContainsPredicate, str> {
        predicate::str::starts_with("<?xml ").and(predicate::str::contains("<svg "))
    }
}
