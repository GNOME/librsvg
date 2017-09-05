use ::cairo;

use error::*;
use parsers::Parse;
use parsers::ParseError;

/// Defines the units to be used for scaling paint servers, per the [svg specification].
///
/// [svg spec]: https://www.w3.org/TR/SVG/pservers.html
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PaintServerUnits {
    UserSpaceOnUse,
    ObjectBoundingBox
}

impl Parse for PaintServerUnits {
    type Data = ();
    type Err = AttributeError;

    fn parse (s: &str, _: ()) -> Result<PaintServerUnits, AttributeError> {
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PaintServerSpread (pub cairo::enums::Extend);

impl Parse for PaintServerSpread {
    type Data = ();
    type Err = AttributeError;

    fn parse (s: &str, _: ()) -> Result <PaintServerSpread, AttributeError> {
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

    #[test]
    fn parsing_invalid_strings_yields_error () {
        assert! (PaintServerUnits::parse ("", ()).is_err ());
        assert! (PaintServerUnits::parse ("foo", ()).is_err ());
    }

    #[test]
    fn parses_paint_server_units () {
        assert_eq! (PaintServerUnits::parse ("userSpaceOnUse", ()), Ok (PaintServerUnits::UserSpaceOnUse));
        assert_eq! (PaintServerUnits::parse ("objectBoundingBox", ()), Ok (PaintServerUnits::ObjectBoundingBox));
    }

    #[test]
    fn parses_spread_method () {
        assert_eq! (PaintServerSpread::parse ("pad", ()),
                    Ok (PaintServerSpread (cairo::enums::Extend::Pad)));

        assert_eq! (PaintServerSpread::parse ("reflect", ()),
                    Ok (PaintServerSpread (cairo::enums::Extend::Reflect)));

        assert_eq! (PaintServerSpread::parse ("repeat", ()),
                    Ok (PaintServerSpread (cairo::enums::Extend::Repeat)));

        assert! (PaintServerSpread::parse ("foobar", ()).is_err ());
    }
}
