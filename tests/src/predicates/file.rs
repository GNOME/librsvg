use predicates::boolean::AndPredicate;
use predicates::prelude::*;
use predicates::str::{ContainsPredicate, StartsWithPredicate};

use crate::predicates::pdf::PdfPredicate;
use crate::predicates::png::PngPredicate;

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
