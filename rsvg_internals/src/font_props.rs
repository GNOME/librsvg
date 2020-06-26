//! CSS font properties.

use cast::{f64, u16};
use cssparser::{Parser, Token};

use crate::drawing_ctx::ViewParams;
use crate::error::*;
use crate::length::*;
use crate::parsers::{finite_f32, Parse};
use crate::properties::ComputedValues;

// https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-size
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FontSize {
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

impl FontSize {
    pub fn value(&self) -> Length<Both> {
        match self {
            FontSize::Value(s) => *s,
            _ => unreachable!(),
        }
    }

    pub fn compute(&self, v: &ComputedValues) -> Self {
        let compute_points = |p| 12.0 * 1.2f64.powf(p) / POINTS_PER_INCH;

        let parent = v.font_size().value();

        // The parent must already have resolved to an absolute unit
        assert!(
            parent.unit != LengthUnit::Percent
                && parent.unit != LengthUnit::Em
                && parent.unit != LengthUnit::Ex
        );

        use FontSize::*;

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

        FontSize::Value(new_size)
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        self.value().normalize(values, params)
    }
}

impl Parse for FontSize {
    #[rustfmt::skip]
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<FontSize, ParseError<'i>> {
        parser
            .try_parse(|p| Length::<Both>::parse(p))
            .and_then(|l| Ok(FontSize::Value(l)))
            .or_else(|_| {
                Ok(parse_identifiers!(
                    parser,
                    "smaller"  => FontSize::Smaller,
                    "larger"   => FontSize::Larger,
                    "xx-small" => FontSize::XXSmall,
                    "x-small"  => FontSize::XSmall,
                    "small"    => FontSize::Small,
                    "medium"   => FontSize::Medium,
                    "large"    => FontSize::Large,
                    "x-large"  => FontSize::XLarge,
                    "xx-large" => FontSize::XXLarge,
                )?)
            })
    }
}

// https://drafts.csswg.org/css-fonts-4/#font-weight-prop
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FontWeight {
    Normal,
    Bold,
    Bolder,
    Lighter,
    Weight(u16),
}

impl Parse for FontWeight {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<FontWeight, ParseError<'i>> {
        parser
            .try_parse(|p| {
                Ok(parse_identifiers!(
                    p,
                    "normal" => FontWeight::Normal,
                    "bold" => FontWeight::Bold,
                    "bolder" => FontWeight::Bolder,
                    "lighter" => FontWeight::Lighter,
                )?)
            })
            .or_else(|_: ParseError| {
                let loc = parser.current_source_location();
                let i = parser.expect_integer()?;
                if (1..=1000).contains(&i) {
                    Ok(FontWeight::Weight(u16(i).unwrap()))
                } else {
                    Err(loc.new_custom_error(ValueErrorKind::value_error(
                        "value must be between 1 and 1000 inclusive",
                    )))
                }
            })
    }
}

impl FontWeight {
    #[rustfmt::skip]
    pub fn compute(&self, v: &Self) -> Self {
        use FontWeight::*;

        // Here, note that we assume that Normal=W400 and Bold=W700, per the spec.  Also,
        // this must match `impl From<FontWeight> for pango::Weight`.
        //
        // See the table at https://drafts.csswg.org/css-fonts-4/#relative-weights

        match *self {
            Bolder => match v.numeric_weight() {
                w if (  1..100).contains(&w) => Weight(400),
                w if (100..350).contains(&w) => Weight(400),
                w if (350..550).contains(&w) => Weight(700),
                w if (550..750).contains(&w) => Weight(900),
                w if (750..900).contains(&w) => Weight(900),
                w if 900 <= w                => Weight(w),

                _ => unreachable!(),
            }

            Lighter => match v.numeric_weight() {
                w if (  1..100).contains(&w) => Weight(w),
                w if (100..350).contains(&w) => Weight(100),
                w if (350..550).contains(&w) => Weight(100),
                w if (550..750).contains(&w) => Weight(400),
                w if (750..900).contains(&w) => Weight(700),
                w if 900 <= w                => Weight(700),

                _ => unreachable!(),
            }

            _ => *self,
        }
    }

    // Converts the symbolic weights to numeric weights.  Will panic on `Bolder` or `Lighter`.
    pub fn numeric_weight(self) -> u16 {
        use FontWeight::*;

        // Here, note that we assume that Normal=W400 and Bold=W700, per the spec.  Also,
        // this must match `impl From<FontWeight> for pango::Weight`.
        match self {
            Normal => 400,
            Bold => 700,
            Bolder | Lighter => unreachable!(),
            Weight(w) => w,
        }
    }
}

// https://www.w3.org/TR/css-text-3/#letter-spacing-property
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LetterSpacing {
    Normal,
    Value(Length<Horizontal>),
}

impl LetterSpacing {
    pub fn value(&self) -> Length<Horizontal> {
        match self {
            LetterSpacing::Value(s) => *s,
            _ => unreachable!(),
        }
    }

    pub fn compute(&self) -> Self {
        let spacing = match self {
            LetterSpacing::Normal => Length::<Horizontal>::new(0.0, LengthUnit::Px),
            LetterSpacing::Value(s) => *s,
        };

        LetterSpacing::Value(spacing)
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        self.value().normalize(values, params)
    }
}

impl Parse for LetterSpacing {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<LetterSpacing, ParseError<'i>> {
        parser
            .try_parse(|p| Length::<Horizontal>::parse(p))
            .and_then(|l| Ok(LetterSpacing::Value(l)))
            .or_else(|_| {
                Ok(parse_identifiers!(
                    parser,
                    "normal" => LetterSpacing::Normal,
                )?)
            })
    }
}

// https://www.w3.org/TR/CSS2/visudet.html#propdef-line-height
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LineHeightSpec {
    Normal,
    Number(f32),
    Length(Length<Both>),
    Percentage(f32),
}

impl LineHeightSpec {
    pub fn value(&self) -> Length<Both> {
        match self {
            LineHeightSpec::Length(l) => *l,
            _ => unreachable!(),
        }
    }

    pub fn compute(&self, values: &ComputedValues) -> Self {
        let font_size = values.font_size().value();

        match *self {
            LineHeightSpec::Normal => LineHeightSpec::Length(font_size),

            LineHeightSpec::Number(f) |
            LineHeightSpec::Percentage(f) => LineHeightSpec::Length(Length::new(font_size.length * f64(f), font_size.unit)),

            LineHeightSpec::Length(l) => LineHeightSpec::Length(l),
        }
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        self.value().normalize(values, params)
    }
}

impl Parse for LineHeightSpec {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<LineHeightSpec, ParseError<'i>> {
        let state = parser.state();
        let loc = parser.current_source_location();

        let token = parser.next()?.clone();

        match token {
            Token::Ident(ref cow) => {
                if cow.eq_ignore_ascii_case("normal") {
                    Ok(LineHeightSpec::Normal)
                } else {
                    Err(parser.new_basic_unexpected_token_error(token.clone()))?
                }
            }

            Token::Number { value, .. } => Ok(LineHeightSpec::Number(finite_f32(value).map_err(|e| loc.new_custom_error(e))?)),

            Token::Percentage { unit_value, .. } => Ok(LineHeightSpec::Percentage(unit_value)),

            Token::Dimension { .. } => {
                parser.reset(&state);
                Ok(LineHeightSpec::Length(Length::<Both>::parse(parser)?))
            }

            _ => {
                Err(parser.new_basic_unexpected_token_error(token))?
            }
        }
    }
}

/// https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-family
#[derive(Debug, Clone, PartialEq)]
pub struct FontFamily(pub String);

impl Parse for FontFamily {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<FontFamily, ParseError<'i>> {
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

        Ok(FontFamily(fonts.join(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::properties::{ParsedProperty, SpecifiedValue, SpecifiedValues};

    #[test]
    fn detects_invalid_invalid_font_size() {
        assert!(FontSize::parse_str("furlong").is_err());
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

        let smaller = FontSize::parse_str("smaller").unwrap().compute(&values);
        if let FontSize::Value(v) = smaller {
            assert!(v.length < 10.0);
            assert_eq!(v.unit, LengthUnit::Px);
        } else {
            unreachable!();
        }

        let larger = FontSize::parse_str("larger").unwrap().compute(&values);
        if let FontSize::Value(v) = larger {
            assert!(v.length > 10.0);
            assert_eq!(v.unit, LengthUnit::Px);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parses_font_weight() {
        assert_eq!(
            <FontWeight as Parse>::parse_str("normal"),
            Ok(FontWeight::Normal)
        );
        assert_eq!(
            <FontWeight as Parse>::parse_str("bold"),
            Ok(FontWeight::Bold)
        );
        assert_eq!(
            <FontWeight as Parse>::parse_str("100"),
            Ok(FontWeight::Weight(100))
        );
    }

    #[test]
    fn detects_invalid_font_weight() {
        assert!(<FontWeight as Parse>::parse_str("").is_err());
        assert!(<FontWeight as Parse>::parse_str("strange").is_err());
        assert!(<FontWeight as Parse>::parse_str("0").is_err());
        assert!(<FontWeight as Parse>::parse_str("-1").is_err());
        assert!(<FontWeight as Parse>::parse_str("1001").is_err());
        assert!(<FontWeight as Parse>::parse_str("3.14").is_err());
    }

    #[test]
    fn parses_letter_spacing() {
        assert_eq!(
            <LetterSpacing as Parse>::parse_str("normal"),
            Ok(LetterSpacing::Normal)
        );
        assert_eq!(
            <LetterSpacing as Parse>::parse_str("10em"),
            Ok(LetterSpacing::Value(Length::<Horizontal>::new(
                10.0,
                LengthUnit::Em,
            )))
        );
    }

    #[test]
    fn computes_letter_spacing() {
        assert_eq!(
            <LetterSpacing as Parse>::parse_str("normal").map(|s| s.compute()),
            Ok(LetterSpacing::Value(Length::<Horizontal>::new(
                0.0,
                LengthUnit::Px,
            )))
        );
        assert_eq!(
            <LetterSpacing as Parse>::parse_str("10em").map(|s| s.compute()),
            Ok(LetterSpacing::Value(Length::<Horizontal>::new(
                10.0,
                LengthUnit::Em,
            )))
        );
    }

    #[test]
    fn detects_invalid_invalid_letter_spacing() {
        assert!(LetterSpacing::parse_str("furlong").is_err());
    }

    #[test]
    fn parses_font_family() {
        assert_eq!(
            <FontFamily as Parse>::parse_str("'Hello world'"),
            Ok(FontFamily("Hello world".to_owned()))
        );

        assert_eq!(
            <FontFamily as Parse>::parse_str("\"Hello world\""),
            Ok(FontFamily("Hello world".to_owned()))
        );

        assert_eq!(
            <FontFamily as Parse>::parse_str("\"Hello world  with  spaces\""),
            Ok(FontFamily("Hello world  with  spaces".to_owned()))
        );

        assert_eq!(
            <FontFamily as Parse>::parse_str("  Hello  world  "),
            Ok(FontFamily("Hello world".to_owned()))
        );

        assert_eq!(
            <FontFamily as Parse>::parse_str("Plonk"),
            Ok(FontFamily("Plonk".to_owned()))
        );
    }

    #[test]
    fn parses_multiple_font_family() {
        assert_eq!(
            <FontFamily as Parse>::parse_str("serif,monospace,\"Hello world\", with  spaces "),
            Ok(FontFamily(
                "serif,monospace,Hello world,with spaces".to_owned()
            ))
        );
    }

    #[test]
    fn detects_invalid_font_family() {
        assert!(<FontFamily as Parse>::parse_str("").is_err());
        assert!(<FontFamily as Parse>::parse_str("''").is_err());
        assert!(<FontFamily as Parse>::parse_str("42").is_err());
    }

    #[test]
    fn parses_line_height() {
        assert_eq!(
            <LineHeightSpec as Parse>::parse_str("normal"),
            Ok(LineHeightSpec::Normal),
        );

        assert_eq!(
            <LineHeightSpec as Parse>::parse_str("2"),
            Ok(LineHeightSpec::Number(2.0)),
        );

        assert_eq!(
            <LineHeightSpec as Parse>::parse_str("2cm"),
            Ok(LineHeightSpec::Length(Length::new(2.0, LengthUnit::Cm))),
        );

        assert_eq!(
            <LineHeightSpec as Parse>::parse_str("150%"),
            Ok(LineHeightSpec::Percentage(1.5)),
        );
    }

    #[test]
    fn detects_invalid_line_height() {
        assert!(<LineHeightSpec as Parse>::parse_str("").is_err());
        assert!(<LineHeightSpec as Parse>::parse_str("florp").is_err());
        assert!(<LineHeightSpec as Parse>::parse_str("3florp").is_err());
    }

    #[test]
    fn computes_line_height() {
        let mut specified = SpecifiedValues::default();
        specified.set_parsed_property(&ParsedProperty::FontSize(SpecifiedValue::Specified(
            FontSize::parse_str("10px").unwrap(),
        )));

        let mut values = ComputedValues::default();
        specified.to_computed_values(&mut values);

        assert_eq!(
            LineHeightSpec::Normal.compute(&values),
            LineHeightSpec::Length(Length::new(10.0, LengthUnit::Px)),
        );

        assert_eq!(
            LineHeightSpec::Number(2.0).compute(&values),
            LineHeightSpec::Length(Length::new(20.0, LengthUnit::Px)),
        );

        assert_eq!(
            LineHeightSpec::Length(Length::new(3.0, LengthUnit::Cm)).compute(&values),
            LineHeightSpec::Length(Length::new(3.0, LengthUnit::Cm)),
        );

        assert_eq!(
            LineHeightSpec::parse_str("150%").unwrap().compute(&values),
            LineHeightSpec::Length(Length::new(15.0, LengthUnit::Px)),
        );
    }
}
