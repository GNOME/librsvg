extern crate phf;

use std::str::FromStr;

include!(concat!(env!("OUT_DIR"), "/attributes-codegen.rs"));

impl FromStr for Attribute {
    type Err = ();

    fn from_str(s: &str) -> Result<Attribute, ()> {
        ATTRIBUTES.get(s).cloned().ok_or(())
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
}
