use std::str::FromStr;

use error::*;
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
    type Err = AttributeError;

    fn from_str (s: &str) -> Result<PaintServerUnits, AttributeError> {
        match s {
            "userSpaceOnUse"    => Ok (PaintServerUnits::UserSpaceOnUse),
            "objectBoundingBox" => Ok (PaintServerUnits::ObjectBoundingBox),
            _                   => Err (AttributeError::Parse (ParseError::new ("expected 'userSpaceOnUse' or 'objectBoundingBox'")))
        }
    }
}

impl Default for PaintServerUnits {
    fn default () -> PaintServerUnits {
        PaintServerUnits::ObjectBoundingBox
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
