const DBL_EPSILON: f64 = 1e-10;
pub const DBL_EPSILON: f64 = 1e-10;

pub fn double_equals (a: f64, b: f64) -> bool {
    (a - b).abs () < DBL_EPSILON
}
