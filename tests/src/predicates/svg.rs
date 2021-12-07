use gio::MemoryInputStream;
use glib::Bytes;
use predicates::prelude::*;
use predicates::reflection::{Case, Child, PredicateReflection, Product};
use std::cmp;
use std::fmt;
use std::str;

use libxml::parser::Parser;
use libxml::xpath::Context;

use librsvg::doctest_only::Both;
use librsvg::rsvg_convert_only::ULength;
use librsvg::{Length, Loader, LoadingError, Parse, SvgHandle};

/// Checks that the variable of type [u8] can be parsed as a SVG file.
#[derive(Debug)]
pub struct SvgPredicate {}

impl SvgPredicate {
    pub fn with_size(self: Self, width: Length, height: Length) -> DetailPredicate<Self> {
        DetailPredicate::<Self> {
            p: self,
            d: Detail::Size(Dimensions {
                w: width,
                h: height,
            }),
        }
    }
}

fn svg_from_bytes(data: &[u8]) -> Result<SvgHandle, LoadingError> {
    let bytes = Bytes::from(data);
    let stream = MemoryInputStream::from_bytes(&bytes);
    Loader::new().read_stream(&stream, None::<&gio::File>, None::<&gio::Cancellable>)
}

impl Predicate<[u8]> for SvgPredicate {
    fn eval(&self, data: &[u8]) -> bool {
        svg_from_bytes(data).is_ok()
    }

    fn find_case<'a>(&'a self, _expected: bool, data: &[u8]) -> Option<Case<'a>> {
        match svg_from_bytes(data) {
            Ok(_) => None,
            Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
        }
    }
}

impl PredicateReflection for SvgPredicate {}

impl fmt::Display for SvgPredicate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "is an SVG")
    }
}

/// Extends a SVG Predicate by a check for its size
#[derive(Debug)]
pub struct DetailPredicate<SvgPredicate> {
    p: SvgPredicate,
    d: Detail,
}

#[derive(Debug)]
enum Detail {
    Size(Dimensions),
}

/// SVG's dimensions
#[derive(Debug)]
struct Dimensions {
    w: Length,
    h: Length,
}

impl Dimensions {
    pub fn width(self: &Self) -> f64 {
        self.w.length
    }

    pub fn height(self: &Self) -> f64 {
        self.h.length
    }
}

impl fmt::Display for Dimensions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{} x {}{}",
            self.width(),
            self.w.unit,
            self.height(),
            self.h.unit
        )
    }
}

impl cmp::PartialEq for Dimensions {
    fn eq(&self, other: &Self) -> bool {
        approx_eq!(f64, self.width(), other.width(), epsilon = 0.000_001)
            && approx_eq!(f64, self.height(), other.height(), epsilon = 0.000_001)
            && (self.w.unit == self.h.unit)
            && (self.h.unit == other.h.unit)
            && (other.h.unit == other.w.unit)
    }
}

impl cmp::Eq for Dimensions {}

trait Details {
    fn get_size(&self) -> Option<Dimensions>;
}

impl DetailPredicate<SvgPredicate> {
    fn eval_doc(&self, doc: &str) -> bool {
        match &self.d {
            Detail::Size(d) => doc.get_size() == Some(Dimensions { w: d.w, h: d.h }),
        }
    }

    fn find_case_for_doc<'a>(&'a self, expected: bool, doc: &str) -> Option<Case<'a>> {
        if self.eval_doc(doc) == expected {
            let product = self.product_for_doc(doc);
            Some(Case::new(Some(self), false).add_product(product))
        } else {
            None
        }
    }

    fn product_for_doc(&self, doc: &str) -> Product {
        match &self.d {
            Detail::Size(_) => Product::new(
                "actual size",
                match doc.get_size() {
                    Some(dim) => format!("{}", dim),
                    None => "None".to_string(),
                },
            ),
        }
    }
}

impl Details for &str {
    fn get_size(self: &Self) -> Option<Dimensions> {
        let parser = Parser::default();
        let doc = parser.parse_string(self).unwrap();
        let context = Context::new(&doc).unwrap();

        let width = context.evaluate("//@width").unwrap().get_nodes_as_vec()[0].get_content();
        let height = context.evaluate("//@height").unwrap().get_nodes_as_vec()[0].get_content();

        let parsed_w = ULength::<Both>::parse_str(&width).unwrap();
        let parsed_h = ULength::<Both>::parse_str(&height).unwrap();

        let dim = Dimensions {
            w: Length::new(parsed_w.length, parsed_w.unit),
            h: Length::new(parsed_h.length, parsed_h.unit),
        };

        return Some(dim);
    }
}

impl Predicate<[u8]> for DetailPredicate<SvgPredicate> {
    fn eval(&self, data: &[u8]) -> bool {
        match str::from_utf8(data) {
            Ok(doc) => self.eval_doc(&doc),
            _ => false,
        }
    }

    fn find_case<'a>(&'a self, expected: bool, data: &[u8]) -> Option<Case<'a>> {
        match str::from_utf8(data) {
            Ok(doc) => self.find_case_for_doc(expected, &doc),
            Err(e) => Some(Case::new(Some(self), false).add_product(Product::new("Error", e))),
        }
    }
}

impl PredicateReflection for DetailPredicate<SvgPredicate> {
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = Child<'a>> + 'a> {
        let params = vec![Child::new("predicate", &self.p)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for DetailPredicate<SvgPredicate> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.d {
            Detail::Size(d) => write!(f, "is an SVG sized {}", d),
        }
    }
}
