extern crate png;

use predicates::prelude::*;
use predicates::reflection::{Case, Child, PredicateReflection, Product};
use std::fmt;

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
