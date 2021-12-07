use predicates::prelude::*;
use predicates::str::StartsWithPredicate;

use crate::predicates::pdf::PdfPredicate;
use crate::predicates::png::PngPredicate;
use crate::predicates::svg::SvgPredicate;

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

pub fn is_svg() -> SvgPredicate {
    SvgPredicate {}
}
