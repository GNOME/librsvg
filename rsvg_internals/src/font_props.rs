//! CSS font properties.

use cssparser::Parser;

use crate::drawing_ctx::ViewParams;
use crate::error::*;
use crate::length::*;
use crate::parsers::Parse;
use crate::properties::ComputedValues;

// https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-size
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
    Value(Length<Both>),
}

impl FontSizeSpec {
    pub fn value(&self) -> Length<Both> {
        match self {
            FontSizeSpec::Value(s) => *s,
            _ => unreachable!(),
        }
    }

    pub fn compute(&self, v: &ComputedValues) -> Self {
        let compute_points = |p| 12.0 * 1.2f64.powf(p) / POINTS_PER_INCH;

        let parent = v.font_size().0.value();

        // The parent must already have resolved to an absolute unit
        assert!(
            parent.unit != LengthUnit::Percent
                && parent.unit != LengthUnit::Em
                && parent.unit != LengthUnit::Ex
        );

        use FontSizeSpec::*;

        #[rustfmt::skip]
        let new_size = match self {
            Smaller => Length::<Both>::new(parent.length / 1.2,  parent.unit),
            Larger  => Length::<Both>::new(parent.length * 1.2,  parent.unit),
            XXSmall => Length::<Both>::new(compute_points(-3.0), LengthUnit::In),
            XSmall  => Length::<Both>::new(compute_points(-2.0), LengthUnit::In),
            Small   => Length::<Both>::new(compute_points(-1.0), LengthUnit::In),
            Medium  => Length::<Both>::new(compute_points(0.0),  LengthUnit::In),
            Large   => Length::<Both>::new(compute_points(1.0),  LengthUnit::In),
            XLarge  => Length::<Both>::new(compute_points(2.0),  LengthUnit::In),
            XXLarge => Length::<Both>::new(compute_points(3.0),  LengthUnit::In),

            Value(s) if s.unit == LengthUnit::Percent => {
                Length::<Both>::new(parent.length * s.length, parent.unit)
            }

            Value(s) if s.unit == LengthUnit::Em => {
                Length::<Both>::new(parent.length * s.length, parent.unit)
            }

            Value(s) if s.unit == LengthUnit::Ex => {
                // FIXME: it would be nice to know the actual Ex-height
                // of the font.
                Length::<Both>::new(parent.length * s.length / 2.0, parent.unit)
            }

            Value(s) => *s,
        };

        FontSizeSpec::Value(new_size)
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        self.value().normalize(values, params)
    }
}

impl Parse for FontSizeSpec {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<FontSizeSpec, ParseError<'i>> {
        parser
            .try_parse(|p| Length::<Both>::parse(p))
            .and_then(|l| Ok(FontSizeSpec::Value(l)))
            .or_else(|_| {
                Ok(parse_identifiers!(
                    parser,
                    "smaller" => FontSizeSpec::Smaller,
                    "larger" => FontSizeSpec::Larger,
                    "xx-small" => FontSizeSpec::XXSmall,
                    "x-small" => FontSizeSpec::XSmall,
                    "small" => FontSizeSpec::Small,
                    "medium" => FontSizeSpec::Medium,
                    "large" => FontSizeSpec::Large,
                    "x-large" => FontSizeSpec::XLarge,
                    "xx-large" => FontSizeSpec::XXLarge,
                )?)
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
    W100,
    W200,
    W300,
    W400,
    W500,
    W600,
    W700,
    W800,
    W900,
}

impl Parse for FontWeightSpec {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<FontWeightSpec, ParseError<'i>> {
        parser
            .try_parse(|p| {
                Ok(parse_identifiers!(
                    p,
                    "normal" => FontWeightSpec::Normal,
                    "bold" => FontWeightSpec::Bold,
                    "bolder" => FontWeightSpec::Bolder,
                    "lighter" => FontWeightSpec::Lighter,
                )?)
            })
            .or_else(|_: ParseError| {
                let loc = parser.current_source_location();
                let i = parser.expect_integer()?;
                match i {
                    100 => Ok(FontWeightSpec::W100),
                    200 => Ok(FontWeightSpec::W200),
                    300 => Ok(FontWeightSpec::W300),
                    400 => Ok(FontWeightSpec::W400),
                    500 => Ok(FontWeightSpec::W500),
                    600 => Ok(FontWeightSpec::W600),
                    700 => Ok(FontWeightSpec::W700),
                    800 => Ok(FontWeightSpec::W800),
                    900 => Ok(FontWeightSpec::W900),
                    _ => Err(loc.new_custom_error(ValueErrorKind::parse_error("parse error"))),
                }
            })
    }
}

// https://www.w3.org/TR/css-text-3/#letter-spacing-property
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LetterSpacingSpec {
    Normal,
    Value(Length<Horizontal>),
}

impl LetterSpacingSpec {
    pub fn value(&self) -> Length<Horizontal> {
        match self {
            LetterSpacingSpec::Value(s) => *s,
            _ => unreachable!(),
        }
    }

    pub fn compute(&self) -> Self {
        let spacing = match self {
            LetterSpacingSpec::Normal => Length::<Horizontal>::new(0.0, LengthUnit::Px),
            LetterSpacingSpec::Value(s) => *s,
        };

        LetterSpacingSpec::Value(spacing)
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        self.value().normalize(values, params)
    }
}

impl Parse for LetterSpacingSpec {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<LetterSpacingSpec, ParseError<'i>> {
        parser
            .try_parse(|p| Length::<Horizontal>::parse(p))
            .and_then(|l| Ok(LetterSpacingSpec::Value(l)))
            .or_else(|_| {
                Ok(parse_identifiers!(
                    parser,
                    "normal" => LetterSpacingSpec::Normal,
                )?)
            })
    }
}

/// https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-family
#[derive(Debug, Clone, PartialEq)]
pub struct MultiFontFamily(pub String);

impl Parse for MultiFontFamily {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<MultiFontFamily, ParseError<'i>> {
        let loc = parser.current_source_location();

        let fonts = parser.parse_comma_separated(|parser| {
            if let Ok(cow) = parser.try_parse(|p| p.expect_string_cloned()) {
                if cow == "" {
                    return Err(loc.new_custom_error(ValueErrorKind::value_error(
                        "empty string is not a valid font family name",
                    )));
                }

                return Ok(cow);
            }

            let first_ident = parser.expect_ident()?.clone();
            let mut value = first_ident.as_ref().to_owned();

            while let Ok(cow) = parser.try_parse(|p| p.expect_ident_cloned()) {
                value.push(' ');
                value.push_str(&cow);
            }
            Ok(cssparser::CowRcStr::from(value))
        })?;

        Ok(MultiFontFamily(fonts.join(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::properties::{ParsedProperty, SpecifiedValue, SpecifiedValues};
    use crate::property_defs::FontSize;
    use crate::property_macros::Property;

    #[test]
    fn detects_invalid_invalid_font_size() {
        assert!(FontSizeSpec::parse_str("furlong").is_err());
    }

    #[test]
    fn computes_parent_relative_font_size() {
        let mut specified = SpecifiedValues::default();
        specified.set_parsed_property(&ParsedProperty::FontSize(SpecifiedValue::Specified(
            FontSize::parse_str("10px").unwrap(),
        )));

        let mut values = ComputedValues::default();
        specified.to_computed_values(&mut values);

        assert_eq!(
            FontSize::parse_str("150%").unwrap().compute(&values),
            FontSize::parse_str("15px").unwrap()
        );

        assert_eq!(
            FontSize::parse_str("1.5em").unwrap().compute(&values),
            FontSize::parse_str("15px").unwrap()
        );

        assert_eq!(
            FontSize::parse_str("1ex").unwrap().compute(&values),
            FontSize::parse_str("5px").unwrap()
        );

        let smaller = FontSize::parse_str("smaller").unwrap().compute(&values).0;
        if let FontSizeSpec::Value(v) = smaller {
            assert!(v.length < 10.0);
            assert_eq!(v.unit, LengthUnit::Px);
        } else {
            unreachable!();
        }

        let larger = FontSize::parse_str("larger").unwrap().compute(&values).0;
        if let FontSizeSpec::Value(v) = larger {
            assert!(v.length > 10.0);
            assert_eq!(v.unit, LengthUnit::Px);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_font_weight() {
        assert_eq!(
            <FontWeightSpec as Parse>::parse_str("normal"),
            Ok(FontWeightSpec::Normal)
        );
        assert_eq!(
            <FontWeightSpec as Parse>::parse_str("bold"),
            Ok(FontWeightSpec::Bold)
        );
        assert_eq!(
            <FontWeightSpec as Parse>::parse_str("100"),
            Ok(FontWeightSpec::W100)
        );
    }

    #[test]
    fn detects_invalid_font_weight() {
        assert!(<FontWeightSpec as Parse>::parse_str("").is_err());
        assert!(<FontWeightSpec as Parse>::parse_str("strange").is_err());
        assert!(<FontWeightSpec as Parse>::parse_str("314").is_err());
        assert!(<FontWeightSpec as Parse>::parse_str("3.14").is_err());
    }

    #[test]
    fn parses_letter_spacing() {
        assert_eq!(
            <LetterSpacingSpec as Parse>::parse_str("normal"),
            Ok(LetterSpacingSpec::Normal)
        );
        assert_eq!(
            <LetterSpacingSpec as Parse>::parse_str("10em"),
            Ok(LetterSpacingSpec::Value(Length::<Horizontal>::new(
                10.0,
                LengthUnit::Em,
            )))
        );
    }

    #[test]
    fn computes_letter_spacing() {
        assert_eq!(
            <LetterSpacingSpec as Parse>::parse_str("normal").map(|s| s.compute()),
            Ok(LetterSpacingSpec::Value(Length::<Horizontal>::new(
                0.0,
                LengthUnit::Px,
            )))
        );
        assert_eq!(
            <LetterSpacingSpec as Parse>::parse_str("10em").map(|s| s.compute()),
            Ok(LetterSpacingSpec::Value(Length::<Horizontal>::new(
                10.0,
                LengthUnit::Em,
            )))
        );
    }

    #[test]
    fn detects_invalid_invalid_letter_spacing() {
        assert!(LetterSpacingSpec::parse_str("furlong").is_err());
    }

    #[test]
    fn parses_font_family() {
        assert_eq!(
            <MultiFontFamily as Parse>::parse_str("'Hello world'"),
            Ok(MultiFontFamily("Hello world".to_owned()))
        );

        assert_eq!(
            <MultiFontFamily as Parse>::parse_str("\"Hello world\""),
            Ok(MultiFontFamily("Hello world".to_owned()))
        );

        assert_eq!(
            <MultiFontFamily as Parse>::parse_str("\"Hello world  with  spaces\""),
            Ok(MultiFontFamily("Hello world  with  spaces".to_owned()))
        );

        assert_eq!(
            <MultiFontFamily as Parse>::parse_str("  Hello  world  "),
            Ok(MultiFontFamily("Hello world".to_owned()))
        );

        assert_eq!(
            <MultiFontFamily as Parse>::parse_str("Plonk"),
            Ok(MultiFontFamily("Plonk".to_owned()))
        );
    }

    #[test]
    fn parses_multiple_font_family() {
        assert_eq!(
            <MultiFontFamily as Parse>::parse_str("serif,monospace,\"Hello world\", with  spaces "),
            Ok(MultiFontFamily("serif,monospace,Hello world,with spaces".to_owned()))
        );
    }

    #[test]
    fn detects_invalid_font_family() {
        assert!(<MultiFontFamily as Parse>::parse_str("").is_err());
        assert!(<MultiFontFamily as Parse>::parse_str("''").is_err());
        assert!(<MultiFontFamily as Parse>::parse_str("42").is_err());
    }
}
