use ::cairo;

use error::*;
use parsers::Parse;
use parsers::ParseError;

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
