use ::cairo;

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

// We define this as a newtype so we can impl FromStr on it
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PaintServerSpread (pub cairo::enums::Extend);

impl FromStr for PaintServerSpread {
    type Err = AttributeError;

    fn from_str (s: &str) -> Result <PaintServerSpread, AttributeError> {
        match s {
            "pad"     => Ok (PaintServerSpread (cairo::enums::Extend::Pad)),
            "reflect" => Ok (PaintServerSpread (cairo::enums::Extend::Reflect)),
            "repeat"  => Ok (PaintServerSpread (cairo::enums::Extend::Repeat)),
            _         => Err (AttributeError::Parse (ParseError::new ("expected 'pad' | 'reflect' | 'repeat'")))
        }
    }
}

impl Default for PaintServerSpread {
    fn default () -> PaintServerSpread {
        PaintServerSpread (cairo::enums::Extend::Pad)
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

    #[test]
    fn parses_spread_method () {
        assert_eq! (PaintServerSpread::from_str ("pad"),
                    Ok (PaintServerSpread (cairo::enums::Extend::Pad)));

        assert_eq! (PaintServerSpread::from_str ("reflect"),
                    Ok (PaintServerSpread (cairo::enums::Extend::Reflect)));

        assert_eq! (PaintServerSpread::from_str ("repeat"),
                    Ok (PaintServerSpread (cairo::enums::Extend::Repeat)));

        assert! (PaintServerSpread::from_str ("foobar").is_err ());
    }
}
