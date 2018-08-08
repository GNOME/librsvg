use cssparser::{Parser, Token};

use drawing_ctx::DrawingCtx;
use error::*;
use length::{Length, LengthDir, LengthUnit, POINTS_PER_INCH};
use parsers::{Parse, ParseError};
use state::ComputedValues;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FontSizeSpec {
    Smaller,
    Larger,
    XXSmall,
    XSmall,
    Small,
    Medium,
    Large,
    XLarge,
    XXLarge,
    Value(Length),
}

impl FontSizeSpec {
    pub fn value(&self) -> Length {
        match self {
            FontSizeSpec::Value(s) => s.clone(),
            _ => unreachable!(),
        }
    }

    pub fn compute(&self, v: &ComputedValues) -> Self {
        let compute_points = |p| 12.0 * 1.2f64.powf(p) / POINTS_PER_INCH;

        let size = v.font_size.0.value();

        let new_size = match self {
            FontSizeSpec::Smaller => Length::new(size.length / 1.2, size.unit, LengthDir::Both),
            FontSizeSpec::Larger => Length::new(size.length * 1.2, size.unit, LengthDir::Both),
            FontSizeSpec::XXSmall => {
                Length::new(compute_points(-3.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::XSmall => {
                Length::new(compute_points(-2.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::Small => {
                Length::new(compute_points(-1.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::Medium => {
                Length::new(compute_points(0.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::Large => {
                Length::new(compute_points(1.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::XLarge => {
                Length::new(compute_points(2.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::XXLarge => {
                Length::new(compute_points(3.0), LengthUnit::Inch, LengthDir::Both)
            }
            FontSizeSpec::Value(s) if s.unit == LengthUnit::Percent => {
                Length::new(size.length * s.length, size.unit, LengthDir::Both)
            }
            FontSizeSpec::Value(s) => s.clone(),
        };

        FontSizeSpec::Value(new_size)
    }

    pub fn normalize(&self, values: &ComputedValues, draw_ctx: &DrawingCtx) -> f64 {
        self.value().normalize(values, draw_ctx)
    }
}

impl Parse for FontSizeSpec {
    type Data = ();
    type Err = AttributeError;

    fn parse(parser: &mut Parser, _: Self::Data) -> Result<FontSizeSpec, ::error::AttributeError> {
        let parser_state = parser.state();

        Length::parse(parser, LengthDir::Both)
            .and_then(|s| Ok(FontSizeSpec::Value(s)))
            .or_else(|e| {
                parser.reset(&parser_state);

                {
                    let token = parser.next().map_err(|_| {
                        ::error::AttributeError::Parse(::parsers::ParseError::new("expected token"))
                    })?;

                    if let Token::Ident(ref cow) = token {
                        match cow.as_ref() {
                            "smaller" => return Ok(FontSizeSpec::Smaller),
                            "larger" => return Ok(FontSizeSpec::Larger),
                            "xx-small" => return Ok(FontSizeSpec::XXSmall),
                            "x-small" => return Ok(FontSizeSpec::XSmall),
                            "small" => return Ok(FontSizeSpec::Small),
                            "medium" => return Ok(FontSizeSpec::Medium),
                            "large" => return Ok(FontSizeSpec::Large),
                            "x-large" => return Ok(FontSizeSpec::XLarge),
                            "xx-large" => return Ok(FontSizeSpec::XXLarge),
                            _ => (),
                        };
                    }
                }

                parser.reset(&parser_state);

                Err(e)
            })
    }
}

// https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-weight
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FontWeightSpec {
    Normal,
    Bold,
    Bolder,
    Lighter,
    W100, // FIXME: we should use Weight(100),
    W200, // but we need a smarter macro for that
    W300,
    W400,
    W500,
    W600,
    W700,
    W800,
    W900,
}

impl Parse for FontWeightSpec {
    type Data = ();
    type Err = AttributeError;

    fn parse(
        parser: &mut Parser,
        _: Self::Data,
    ) -> Result<FontWeightSpec, ::error::AttributeError> {
        if let Ok(r) = parser.try(|p| {
            p.expect_ident()
                .map_err(|_| ())
                .and_then(|cow| match cow.as_ref() {
                    "normal" => Ok(FontWeightSpec::Normal),
                    "bold" => Ok(FontWeightSpec::Bold),
                    "bolder" => Ok(FontWeightSpec::Bolder),
                    "lighter" => Ok(FontWeightSpec::Lighter),
                    _ => Err(()),
                })
        }) {
            return Ok(r);
        }

        if let Ok(r) = parser
            .expect_integer()
            .map_err(|_| ())
            .and_then(|i| match i {
                100 => Ok(FontWeightSpec::W100),
                200 => Ok(FontWeightSpec::W200),
                300 => Ok(FontWeightSpec::W300),
                400 => Ok(FontWeightSpec::W400),
                500 => Ok(FontWeightSpec::W500),
                600 => Ok(FontWeightSpec::W600),
                700 => Ok(FontWeightSpec::W700),
                800 => Ok(FontWeightSpec::W800),
                900 => Ok(FontWeightSpec::W900),
                _ => Err(()),
            }) {
            Ok(r)
        } else {
            Err(AttributeError::Parse(ParseError::new(
                "invalid font-weight specification",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_font_size_yields_error() {
        assert!(is_parse_error(&FontSizeSpec::parse_str("furlong", ())));
    }

    #[test]
    fn parses_font_weight() {
        assert_eq!(
            <FontWeightSpec as Parse>::parse_str("normal", ()),
            Ok(FontWeightSpec::Normal)
        );
        assert_eq!(
            <FontWeightSpec as Parse>::parse_str("bold", ()),
            Ok(FontWeightSpec::Bold)
        );
        assert_eq!(
            <FontWeightSpec as Parse>::parse_str("100", ()),
            Ok(FontWeightSpec::W100)
        );
    }

    #[test]
    fn detects_invalid_font_weight() {
        assert!(<FontWeightSpec as Parse>::parse_str("", ()).is_err());
        assert!(<FontWeightSpec as Parse>::parse_str("strange", ()).is_err());
        assert!(<FontWeightSpec as Parse>::parse_str("314", ()).is_err());
        assert!(<FontWeightSpec as Parse>::parse_str("3.14", ()).is_err());
    }
}
