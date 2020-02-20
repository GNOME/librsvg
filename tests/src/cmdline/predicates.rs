extern crate chrono;
extern crate lopdf;
extern crate png;
extern crate predicates;

pub mod file {

    use chrono::{DateTime, Utc};

    use predicates::boolean::AndPredicate;
    use predicates::prelude::*;
    use predicates::reflection::{Case, Child, PredicateReflection, Product};
    use predicates::str::{ContainsPredicate, StartsWithPredicate};

    use std::cmp;
    use std::fmt;

    /// Checks that the variable of type [u8] can be parsed as a PDF file.
    #[derive(Debug)]
    pub struct PdfPredicate {}

    impl PdfPredicate {
        pub fn with_page_count(self: Self, num_pages: usize) -> DetailPredicate<Self> {
            DetailPredicate::<Self> {
                p: self,
                d: Detail::PageCount(num_pages),
            }
        }

        pub fn with_page_size(
            self: Self,
            width: i64,
            height: i64,
            dpi: f64,
        ) -> DetailPredicate<Self> {
            DetailPredicate::<Self> {
                p: self,
                d: Detail::PageSize(Dimensions {
                    w: width,
                    h: height,
                    unit: dpi / 72.0,
                }),
            }
        }

        pub fn with_creation_date(self: Self, when: DateTime<Utc>) -> DetailPredicate<Self> {
            DetailPredicate::<Self> {
                p: self,
                d: Detail::CreationDate(when),
            }
        }
    }

    impl Predicate<[u8]> for PdfPredicate {
        fn eval(&self, data: &[u8]) -> bool {
            lopdf::Document::load_mem(data).is_ok()
        }

        fn find_case<'a>(&'a self, _expected: bool, data: &[u8]) -> Option<Case<'a>> {
            match lopdf::Document::load_mem(data) {
                Ok(_) => None,
                Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
            }
        }
    }

    impl PredicateReflection for PdfPredicate {}

    impl fmt::Display for PdfPredicate {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "is a PDF")
        }
    }

    /// Extends a PdfPredicate by a check for page count, page size or creation date.
    #[derive(Debug)]
    pub struct DetailPredicate<PdfPredicate> {
        p: PdfPredicate,
        d: Detail,
    }

    #[derive(Debug)]
    enum Detail {
        PageCount(usize),
        PageSize(Dimensions),
        CreationDate(DateTime<Utc>),
    }

    #[derive(Debug)]
    struct Dimensions {
        w: i64,
        h: i64,    //
        unit: f64, // UserUnit, in 1/72 of an inch
    }

    impl Dimensions {
        pub fn from_media_box(obj: &lopdf::Object, unit: Option<f64>) -> lopdf::Result<Dimensions> {
            let a = obj.as_array()?;
            Ok(Dimensions {
                w: a[2].as_i64()?,
                h: a[3].as_i64()?,
                unit: unit.unwrap_or(1.0),
            })
        }

        pub fn width_in_mm(self: &Self) -> f64 {
            self.w as f64 * self.unit * 72.0 / 254.0
        }

        pub fn height_in_mm(self: &Self) -> f64 {
            self.h as f64 * self.unit * 72.0 / 254.0
        }
    }

    impl fmt::Display for Dimensions {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{} mm x {} mm", self.width_in_mm(), self.height_in_mm())
        }
    }

    impl cmp::PartialEq for Dimensions {
        fn eq(&self, other: &Self) -> bool {
            approx_eq!(f64, self.width_in_mm(), other.width_in_mm())
                && approx_eq!(f64, self.height_in_mm(), other.height_in_mm())
        }
    }

    impl cmp::Eq for Dimensions {}

    trait Details {
        fn get_page_count(&self) -> usize;
        fn get_page_size(&self) -> Option<Dimensions>;
        fn get_creation_date(&self) -> Option<DateTime<Utc>>;
        fn get_from_trailer<'a>(self: &'a Self, key: &[u8]) -> lopdf::Result<&'a lopdf::Object>;
        fn get_from_first_page<'a>(self: &'a Self, key: &[u8]) -> lopdf::Result<&'a lopdf::Object>;
    }

    impl DetailPredicate<PdfPredicate> {
        fn eval_doc(&self, doc: &lopdf::Document) -> bool {
            match &self.d {
                Detail::PageCount(n) => doc.get_page_count() == *n,
                Detail::PageSize(d) => doc.get_page_size().map_or(false, |dim| dim == *d),
                Detail::CreationDate(d) => doc.get_creation_date().map_or(false, |date| date == *d),
            }
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
            match &self.d {
                Detail::PageCount(_) => Product::new(
                    "actual page count",
                    format!("{} page(s)", doc.get_page_count()),
                ),
                Detail::PageSize(_) => Product::new(
                    "actual page size",
                    match doc.get_page_size() {
                        Some(dim) => format!("{}", dim),
                        None => "None".to_string(),
                    },
                ),
                Detail::CreationDate(_) => Product::new(
                    "actual creation date",
                    format!("{:?}", doc.get_creation_date()),
                ),
            }
        }
    }

    impl Details for lopdf::Document {
        fn get_page_count(self: &Self) -> usize {
            self.get_pages().len()
        }

        fn get_page_size(self: &Self) -> Option<Dimensions> {
            let to_f64 = |obj: &lopdf::Object| obj.as_f64();
            match self.get_from_first_page(b"MediaBox") {
                Ok(obj) => {
                    let unit = self.get_from_first_page(b"UserUnit").and_then(to_f64).ok();
                    Dimensions::from_media_box(obj, unit).ok()
                }
                Err(_) => None,
            }
        }

        fn get_creation_date(self: &Self) -> Option<DateTime<Utc>> {
            match self.get_from_trailer(b"CreationDate") {
                Ok(obj) => obj.as_datetime().map(|date| date.with_timezone(&Utc)),
                Err(_) => None,
            }
        }

        fn get_from_trailer<'a>(self: &'a Self, key: &[u8]) -> lopdf::Result<&'a lopdf::Object> {
            let id = self.trailer.get(b"Info")?.as_reference()?;
            self.get_object(id)?.as_dict()?.get(key)
        }

        fn get_from_first_page<'a>(self: &'a Self, key: &[u8]) -> lopdf::Result<&'a lopdf::Object> {
            match self.page_iter().next() {
                Some(id) => self.get_object(id)?.as_dict()?.get(key),
                None => Err(lopdf::Error::ObjectNotFound),
            }
        }
    }

    impl Predicate<[u8]> for DetailPredicate<PdfPredicate> {
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

    impl PredicateReflection for DetailPredicate<PdfPredicate> {
        fn children<'a>(&'a self) -> Box<dyn Iterator<Item = Child<'a>> + 'a> {
            let params = vec![Child::new("predicate", &self.p)];
            Box::new(params.into_iter())
        }
    }

    impl fmt::Display for DetailPredicate<PdfPredicate> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match &self.d {
                Detail::PageCount(n) => write!(f, "is a PDF with {} page(s)", n),
                Detail::PageSize(d) => write!(f, "is a PDF sized {}", d),
                Detail::CreationDate(d) => write!(f, "is a PDF created {:?}", d),
            }
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
