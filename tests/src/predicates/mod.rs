extern crate predicates;

pub mod file;
mod pdf;
mod png;

use predicates::str;

pub fn ends_with_pkg_version() -> str::EndsWithPredicate {
    str::ends_with(env!("CARGO_PKG_VERSION"))
}
