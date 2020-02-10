extern crate lopdf;
extern crate png;
extern crate predicates;

pub mod file {

    use predicates::boolean::AndPredicate;
    use predicates::prelude::*;
    use predicates::reflection::{Case, Child, PredicateReflection, Product};
    use predicates::str::{ContainsPredicate, StartsWithPredicate};

    use std::fmt;

    /// Checks that the variable of type [u8] can be parsed as a PDF file.
    #[derive(Debug)]
    pub struct PdfPredicate {}

    impl PdfPredicate {
        pub fn with_page_count(self: Self, num_pages: usize) -> PageCountPredicate<Self> {
            PageCountPredicate::<Self> { p: self, n: num_pages }
        }
    }

    impl Predicate<[u8]> for PdfPredicate {
        fn eval(&self, data: &[u8]) -> bool {
            lopdf::Document::load_mem(data).is_ok()
        }

        fn find_case<'a>(&'a self, _expected: bool, data: &[u8]) -> Option<Case<'a>> {
            match lopdf::Document::load_mem(data) {
                Ok(_) => None,
                Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e)))
            }
        }
    }

    impl PredicateReflection for PdfPredicate {}

    impl fmt::Display for PdfPredicate {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "is a PDF")
        }
    }

    /// Extends a PdfPredicate by a check for a given number of pages.
    #[derive(Debug)]
    pub struct PageCountPredicate<PdfPredicate> {
        p: PdfPredicate,
        n: usize
    }

    impl PageCountPredicate<PdfPredicate> {
        fn eval_doc(&self, doc: &lopdf::Document) -> bool {
            doc.get_pages().len() == self.n
        }

        fn find_case_for_doc<'a>(
            &'a self,
            expected: bool,
            doc: &lopdf::Document,
        ) -> Option<Case<'a>> {
            if self.eval_doc(doc) == expected {
                let product = self.product_for_doc(doc);
                Some(Case::new(Some(self), false).add_product(product))
            } else {
                None
            }
        }

        fn product_for_doc(&self, doc: &lopdf::Document) -> Product {
            let actual_count = format!("{} page(s)", doc.get_pages().len());
            Product::new("actual page count", actual_count)
        }
    }

    impl Predicate<[u8]> for PageCountPredicate<PdfPredicate> {
        fn eval(&self, data: &[u8]) -> bool {
            match lopdf::Document::load_mem(data) {
                Ok(doc) => self.eval_doc(&doc),
                _ => false,
            }
        }

        fn find_case<'a>(&'a self, expected: bool, data: &[u8]) -> Option<Case<'a>> {
            match lopdf::Document::load_mem(data) {
                Ok(doc) => self.find_case_for_doc(expected, &doc),
                Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
            }
        }
    }

    impl PredicateReflection for PageCountPredicate<PdfPredicate> {
        fn children<'a>(&'a self) -> Box<dyn Iterator<Item = Child<'a>> + 'a> {
            let params = vec![Child::new("predicate", &self.p)];
            Box::new(params.into_iter())
        }
    }

    impl fmt::Display for PageCountPredicate<PdfPredicate> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "is a PDF with {} page(s)", self.n)
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
