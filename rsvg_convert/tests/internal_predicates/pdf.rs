use chrono::{DateTime, Local, Utc};
use float_cmp::approx_eq;
use lopdf::{self, Dictionary, Object};
use predicates::prelude::*;
use predicates::reflection::{Case, Child, PredicateReflection, Product};
use std::cmp;
use std::fmt;

/// Checks that the variable of type [u8] can be parsed as a PDF file.
#[derive(Debug)]
pub struct PdfPredicate {}

impl PdfPredicate {
    pub fn with_page_count(self, num_pages: usize) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::PageCount(num_pages),
        }
    }

    pub fn with_page_size(
        self,
        idx: usize,
        width_in_points: f32,
        height_in_points: f32,
    ) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::PageSize(
                Dimensions {
                    w: width_in_points,
                    h: height_in_points,
                    unit: 1.0,
                },
                idx,
            ),
        }
    }

    pub fn with_creation_date(self, when: DateTime<Utc>) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::CreationDate(when),
        }
    }

    pub fn with_link(self, link: &str) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::Link(link.to_string()),
        }
    }

    pub fn with_text(self, text: &str) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::Text(text.to_string()),
        }
    }

    pub fn with_version(self, version: &str) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::Version(version.to_string()),
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
    PageSize(Dimensions, usize),
    CreationDate(DateTime<Utc>),
    Link(String),
    Text(String),
    Version(String),
}

/// A PDF page's dimensions from its `MediaBox`.
///
/// Note that `w` and `h` given in `UserUnit`, which is by default 1.0 = 1/72 inch.
#[derive(Debug)]
struct Dimensions {
    w: f32,
    h: f32,
    unit: f32, // UserUnit, in points (1/72 of an inch)
}

impl Dimensions {
    pub fn from_media_box(obj: &lopdf::Object, unit: Option<f32>) -> lopdf::Result<Dimensions> {
        let a = obj.as_array()?;
        Ok(Dimensions {
            w: a[2].as_float()?,
            h: a[3].as_float()?,
            unit: unit.unwrap_or(1.0),
        })
    }

    pub fn width_in_pt(&self) -> f32 {
        self.w * self.unit
    }

    pub fn height_in_pt(&self) -> f32 {
        self.h * self.unit
    }
}

impl fmt::Display for Dimensions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} pt x {} pt", self.width_in_pt(), self.height_in_pt())
    }
}

impl cmp::PartialEq for Dimensions {
    fn eq(&self, other: &Self) -> bool {
        approx_eq!(
            f32,
            self.width_in_pt(),
            other.width_in_pt(),
            epsilon = 0.0001
        ) && approx_eq!(
            f32,
            self.height_in_pt(),
            other.height_in_pt(),
            epsilon = 0.0001
        )
    }
}

impl cmp::Eq for Dimensions {}

trait Details {
    fn get_page_count(&self) -> usize;
    fn get_page_size(&self, idx: usize) -> Option<Dimensions>;
    fn get_creation_date(&self) -> Option<DateTime<Utc>>;
    fn get_from_trailer<'a>(&'a self, key: &[u8]) -> lopdf::Result<&'a lopdf::Object>;
    fn get_from_page<'a>(&'a self, idx: usize, key: &[u8]) -> lopdf::Result<&'a lopdf::Object>;
}

impl DetailPredicate<PdfPredicate> {
    fn eval_doc(&self, doc: &lopdf::Document) -> bool {
        match &self.d {
            Detail::PageCount(n) => doc.get_page_count() == *n,
            Detail::PageSize(d, idx) => doc.get_page_size(*idx).map_or(false, |dim| dim == *d),
            Detail::CreationDate(d) => doc.get_creation_date().map_or(false, |date| date == *d),
            Detail::Link(link) => document_has_link(doc, link),
            Detail::Text(text) => document_has_text(doc, text),
            Detail::Version(version) => document_has_version(doc, version),
        }
    }

    fn find_case_for_doc<'a>(&'a self, expected: bool, doc: &lopdf::Document) -> Option<Case<'a>> {
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
            Detail::PageSize(_, idx) => Product::new(
                "actual page size",
                match doc.get_page_size(*idx) {
                    Some(dim) => format!("{}", dim),
                    None => "None".to_string(),
                },
            ),
            Detail::CreationDate(_) => Product::new(
                "actual creation date",
                format!("{:?}", doc.get_creation_date()),
            ),
            Detail::Link(_) => Product::new(
                "actual link contents",
                "FIXME: who knows, but it's not what we expected".to_string(),
            ),
            Detail::Text(_) => {
                Product::new("actual text contents", doc.extract_text(&[1]).unwrap())
            }
            Detail::Version(_) => Product::new("actual version contents", doc.version.to_string()),
        }
    }
}

impl Details for lopdf::Document {
    fn get_page_count(&self) -> usize {
        self.get_pages().len()
    }

    fn get_page_size(&self, idx: usize) -> Option<Dimensions> {
        match self.get_from_page(idx, b"MediaBox") {
            Ok(obj) => {
                let unit = self
                    .get_from_page(idx, b"UserUnit")
                    .and_then(Object::as_float)
                    .ok();
                Dimensions::from_media_box(obj, unit).ok()
            }
            Err(_) => None,
        }
    }

    fn get_creation_date(&self) -> Option<DateTime<Utc>> {
        match self.get_from_trailer(b"CreationDate") {
            Ok(obj) => match obj.as_datetime() {
                Some(d) => {
                    let local_datetime = DateTime::<Local>::try_from(d).ok()?;
                    Some(local_datetime.into())
                }

                None => None,
            },

            Err(_) => None,
        }
    }

    fn get_from_trailer<'a>(&'a self, key: &[u8]) -> lopdf::Result<&'a lopdf::Object> {
        let id = self.trailer.get(b"Info")?.as_reference()?;
        self.get_object(id)?.as_dict()?.get(key)
    }

    fn get_from_page<'a>(&'a self, idx: usize, key: &[u8]) -> lopdf::Result<&'a lopdf::Object> {
        let mut iter = self.page_iter();
        for _ in 0..idx {
            let _ = iter.next();
        }
        match iter.next() {
            Some(id) => self.get_object(id)?.as_dict()?.get(key),
            None => Err(lopdf::Error::PageNumberNotFound(idx as u32)),
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
            Detail::PageSize(d, _) => write!(f, "is a PDF sized {}", d),
            Detail::CreationDate(d) => write!(f, "is a PDF created {:?}", d),
            Detail::Link(l) => write!(f, "is a PDF with a link to {}", l),
            Detail::Text(t) => write!(f, "is a PDF with \"{}\" in its text content", t),
            Detail::Version(v) => write!(f, "is a PDF with version {}", v),
        }
    }
}

// This is an extremely trivial test for a string being present in the document's
// text objects.
fn document_has_text(document: &lopdf::Document, needle: &str) -> bool {
    if let Ok(haystack) = text_from_first_page(document) {
        haystack.contains(needle)
    } else {
        false
    }
}

fn document_has_version(document: &lopdf::Document, version_to_search: &str) -> bool {
    document.version == version_to_search
}

// We do a super simple test that a PDF actually contains an Annotation object
// with a particular link.  We don't test that this annotation is actually linked
// from a page; that would be nicer.
fn document_has_link(document: &lopdf::Document, link_text: &str) -> bool {
    document
        .objects
        .values()
        .any(|obj| object_is_annotation_with_link(obj, link_text))
}

fn object_is_annotation_with_link(object: &Object, link_text: &str) -> bool {
    object
        .as_dict()
        .map(|dict| dict_is_annotation(dict) && dict_has_a_with_link(dict, link_text))
        .unwrap_or(false)
}

fn dict_is_annotation(dict: &Dictionary) -> bool {
    dict.get(b"Type")
        .and_then(|type_val| type_val.as_name())
        .map(|name| name == b"Annot")
        .unwrap_or(false)
}

fn dict_has_a_with_link(dict: &Dictionary, link_text: &str) -> bool {
    dict.get(b"A")
        .and_then(|obj| obj.as_dict())
        .and_then(|dict| dict.get(b"URI"))
        .and_then(|obj| obj.as_str())
        .map(|string| string == link_text.as_bytes())
        .unwrap_or(false)
}

fn text_from_first_page(doc: &lopdf::Document) -> lopdf::Result<String> {
    // This is extremely simplistic; lopdf just concatenates all the text in the page
    // into a single string.
    doc.extract_text(&[1])
}
