use std::str::FromStr;

use parsers::ParseError;

/// Defines the units to be used for scaling paint servers, per the [svg specification].
///
/// [svg spec]: https://www.w3.org/TR/SVG/pservers.html
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PaintServerUnits {
    UserSpaceOnUse,
    ObjectBoundingBox
}

impl FromStr for PaintServerUnits {
    type Err = ParseError;

    fn from_str (s: &str) -> Result<PaintServerUnits, ParseError> {
        match s {
            "userSpaceOnUse"    => Ok (PaintServerUnits::UserSpaceOnUse),
            "objectBoundingBox" => Ok (PaintServerUnits::ObjectBoundingBox),
            _                   => Err (ParseError::new ("expected 'userSpaceOnUse' or 'objectBoundingBox'"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parsing_invalid_strings_yields_error () {
        assert! (PaintServerUnits::from_str ("").is_err ());
        assert! (PaintServerUnits::from_str ("foo").is_err ());
    }

    #[test]
    fn parses_paint_server_units () {
        assert_eq! (PaintServerUnits::from_str ("userSpaceOnUse"), Ok (PaintServerUnits::UserSpaceOnUse));
        assert_eq! (PaintServerUnits::from_str ("objectBoundingBox"), Ok (PaintServerUnits::ObjectBoundingBox));
    }
}
