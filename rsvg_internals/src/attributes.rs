extern crate phf;

use std::fmt;
use std::str::FromStr;

include!(concat!(env!("OUT_DIR"), "/attributes-codegen.rs"));

impl FromStr for Attribute {
    type Err = ();

    fn from_str(s: &str) -> Result<Attribute, ()> {
        ATTRIBUTES.get(s).cloned().ok_or(())
    }
}

impl fmt::Display for Attribute {
    // This is horribly inefficient, but for now I'm too lazy to have a
    // compile-time bijective mapping from attributes to names.  Hopefully
    // this function is only called when *printing* errors, which, uh,
    // should not be done too often.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (k, v) in ATTRIBUTES.entries() {
            if *v == *self {
                return write!(f, "{}", k);
            }
        }

        unreachable!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes() {
        assert_eq!(Attribute::from_str("width"), Ok(Attribute::Width));
    }

    #[test]
    fn unknown_attribute_yields_error() {
        assert_eq!(Attribute::from_str("foobar"), Err(()));
    }

    #[test]
    fn converts_attributes_back_to_strings() {
        assert_eq!(Attribute::ClipPath.to_string(), "clip-path");
        assert_eq!(Attribute::KernelUnitLength.to_string(), "kernelUnitLength");
        assert_eq!(Attribute::Offset.to_string(), "offset");
    }
}
