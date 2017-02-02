// In paint servers (patterns, gradients, etc.), we have an
// Option<String> for fallback names.  This is a utility function to
// clone one of those.
pub fn clone_fallback_name (fallback: &Option<String>) -> Option<String> {
    if let Some (ref fallback_name) = *fallback {
        Some (fallback_name.clone ())
    } else {
        None
    }
}

pub const DBL_EPSILON: f64 = 1e-10;

pub fn double_equals (a: f64, b: f64) -> bool {
    (a - b).abs () < DBL_EPSILON
}
