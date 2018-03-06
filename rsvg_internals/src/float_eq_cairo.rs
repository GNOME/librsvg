// The following are copied from cairo/src/{cairo-fixed-private.h, cairo-fixed-type-private.h}

const CAIRO_FIXED_FRAC_BITS: u64 = 8;
const CAIRO_MAGIC_NUMBER_FIXED: f64 = (1u64 << (52 - CAIRO_FIXED_FRAC_BITS)) as f64 * 1.5;

fn cairo_magic_double(d: f64) -> f64 {
    d + CAIRO_MAGIC_NUMBER_FIXED
}

fn cairo_fixed_from_double(d: f64) -> i32 {
    let bits = cairo_magic_double(d).to_bits();
    let lower = bits & 0xffffffff;
    lower as i32
}

/// Implements a method to check whether two `f64` numbers would have
/// the same fixed-point representation in Cairo.
///
/// This generally means that the absolute difference between them,
/// when taken as floating-point numbers, is less than the smallest
/// representable fraction that Cairo can represent in fixed-point.
///
/// Implementation detail: Cairo fixed-point numbers use 24 bits for
/// the integral part, and 8 bits for the fractional part.  That is,
/// the smallest fraction they can represent is 1/256.
pub trait FixedEqCairo {
    fn fixed_eq_cairo(&self, other: &Self) -> bool;
}

impl FixedEqCairo for f64 {
    fn fixed_eq_cairo(&self, other: &f64) -> bool {
        // FIXME: Here we have the same problem as Cairo itself: we
        // don't check for overflow in the conversion of double to
        // fixed-point.
        cairo_fixed_from_double(*self) == cairo_fixed_from_double(*other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_equal_in_cairo_fixed_point() {
        assert!(1.0_f64.fixed_eq_cairo(&1.0_f64));

        assert!(1.0_f64.fixed_eq_cairo(&1.001953125_f64)); // 1 + 1/512 - cairo rounds to 1

        assert!(!1.0_f64.fixed_eq_cairo(&1.00390625_f64)); // 1 + 1/256 - cairo can represent it
    }
}
