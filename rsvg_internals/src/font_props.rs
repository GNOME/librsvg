//! CSS font properties.

use cast::{f64, u16};
use cssparser::{Parser, Token};

use crate::drawing_ctx::ViewParams;
use crate::error::*;
use crate::length::*;
use crate::parsers::{finite_f32, Parse};
use crate::properties::ComputedValues;
use crate::property_defs::{FontStretch, FontStyle, FontVariant};

// https://www.w3.org/TR/CSS2/fonts.html#font-shorthand
// https://drafts.csswg.org/css-fonts-4/#font-prop
// servo/components/style/properties/shorthands/font.mako.rs is a good reference for this
#[derive(Debug, Clone, PartialEq)]
pub enum Font {
    Caption,
    Icon,
    Menu,
    MessageBox,
    SmallCaption,
    StatusBar,
    Spec(FontSpec),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FontSpec {
    pub style: FontStyle,
    pub variant: FontVariant,
    pub weight: FontWeight,
    pub stretch: FontStretch,
    pub size: FontSize,
    pub line_height: LineHeight,
    pub family: FontFamily,
}

impl Parse for Font {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Font, ParseError<'i>> {
        if let Ok(f) = parse_font_spec_identifiers(parser) {
            return Ok(f);
        }

        // The following is stolen from servo/components/style/properties/shorthands/font.mako.rs

        let mut nb_normals = 0;
        let mut style = None;
        let mut variant_caps = None;
        let mut weight = None;
        let mut stretch = None;
        let size;

        loop {
            // Special-case 'normal' because it is valid in each of
            // font-style, font-weight, font-variant and font-stretch.
            // Leaves the values to None, 'normal' is the initial value for each of them.
            if parser
                .try_parse(|input| input.expect_ident_matching("normal"))
                .is_ok()
            {
                nb_normals += 1;
                continue;
            }
            if style.is_none() {
                if let Ok(value) = parser.try_parse(FontStyle::parse) {
                    style = Some(value);
                    continue;
                }
            }
            if weight.is_none() {
                if let Ok(value) = parser.try_parse(FontWeight::parse) {
                    weight = Some(value);
                    continue;
                }
            }
            if variant_caps.is_none() {
                if let Ok(value) = parser.try_parse(FontVariant::parse) {
                    variant_caps = Some(value);
                    continue;
                }
            }
            if stretch.is_none() {
                if let Ok(value) = parser.try_parse(FontStretch::parse) {
                    stretch = Some(value);
                    continue;
                }
            }
            size = FontSize::parse(parser)?;
            break;
        }

        let line_height = if parser.try_parse(|input| input.expect_delim('/')).is_ok() {
            Some(LineHeight::parse(parser)?)
        } else {
            None
        };

        #[inline]
        fn count<T>(opt: &Option<T>) -> u8 {
            if opt.is_some() {
                1
            } else {
                0
            }
        }

        if (count(&style) + count(&weight) + count(&variant_caps) + count(&stretch) + nb_normals)
            > 4
        {
            return Err(parser.new_custom_error(ValueErrorKind::parse_error(
                "invalid syntax for 'font' property",
            )));
        }

        let family = FontFamily::parse(parser)?;

        Ok(Font::Spec(FontSpec {
            style: style.unwrap_or_default(),
            variant: variant_caps.unwrap_or_default(),
            weight: weight.unwrap_or_default(),
            stretch: stretch.unwrap_or_default(),
            size,
            line_height: line_height.unwrap_or_default(),
            family,
        }))
    }
}

impl Font {
    pub fn to_font_spec(&self) -> FontSpec {
        match *self {
            Font::Caption
            | Font::Icon
            | Font::Menu
            | Font::MessageBox
            | Font::SmallCaption
            | Font::StatusBar => {
                // We don't actually pick up the systme fonts, so reduce them to a default.
                FontSpec::default()
            }

            Font::Spec(ref spec) => spec.clone(),
        }
    }
}

#[rustfmt::skip]
fn parse_font_spec_identifiers<'i>(parser: &mut Parser<'i, '_>) -> Result<Font, ParseError<'i>> {
    Ok(parser.try_parse(|p| {
        parse_identifiers! {
            p,
            "caption"       => Font::Caption,
            "icon"          => Font::Icon,
            "menu"          => Font::Menu,
            "message-box"   => Font::MessageBox,
            "small-caption" => Font::SmallCaption,
            "status-bar"    => Font::StatusBar,
        }
    })?)
}

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
pub enum LineHeight {
    Normal,
    Number(f32),
    Length(Length<Both>),
    Percentage(f32),
}

impl LineHeight {
    pub fn value(&self) -> Length<Both> {
        match self {
            LineHeight::Length(l) => *l,
            _ => unreachable!(),
        }
    }

    pub fn compute(&self, values: &ComputedValues) -> Self {
        let font_size = values.font_size().value();

        match *self {
            LineHeight::Normal => LineHeight::Length(font_size),

            LineHeight::Number(f) | LineHeight::Percentage(f) => {
                LineHeight::Length(Length::new(font_size.length * f64(f), font_size.unit))
            }

            LineHeight::Length(l) => LineHeight::Length(l),
        }
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        self.value().normalize(values, params)
    }
}

impl Parse for LineHeight {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<LineHeight, ParseError<'i>> {
        let state = parser.state();
        let loc = parser.current_source_location();

        let token = parser.next()?.clone();

        match token {
            Token::Ident(ref cow) => {
                if cow.eq_ignore_ascii_case("normal") {
                    Ok(LineHeight::Normal)
                } else {
                    Err(parser.new_basic_unexpected_token_error(token.clone()))?
                }
            }

            Token::Number { value, .. } => Ok(LineHeight::Number(
                finite_f32(value).map_err(|e| loc.new_custom_error(e))?,
            )),

            Token::Percentage { unit_value, .. } => Ok(LineHeight::Percentage(unit_value)),

            Token::Dimension { .. } => {
                parser.reset(&state);
                Ok(LineHeight::Length(Length::<Both>::parse(parser)?))
            }

            _ => Err(parser.new_basic_unexpected_token_error(token))?,
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

impl FontFamily {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::properties::{ParsedProperty, SpecifiedValue, SpecifiedValues};

    #[test]
    fn parses_font_shorthand() {
        assert_eq!(
            Font::parse_str("small-caption").unwrap(),
            Font::SmallCaption,
        );

        assert_eq!(
            Font::parse_str("italic bold 12px sans").unwrap(),
            Font::Spec(FontSpec {
                style: FontStyle::Italic,
                variant: Default::default(),
                weight: FontWeight::Bold,
                stretch: Default::default(),
                size: FontSize::Value(Length::new(12.0, LengthUnit::Px)),
                line_height: Default::default(),
                family: FontFamily("sans".to_string()),
            }),
        );

        assert_eq!(
            Font::parse_str("bold 14cm/2 serif").unwrap(),
            Font::Spec(FontSpec {
                style: Default::default(),
                variant: Default::default(),
                weight: FontWeight::Bold,
                stretch: Default::default(),
                size: FontSize::Value(Length::new(14.0, LengthUnit::Cm)),
                line_height: LineHeight::Number(2.0),
                family: FontFamily("serif".to_string()),
            }),
        );
    }

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
            <LineHeight as Parse>::parse_str("normal"),
            Ok(LineHeight::Normal),
        );

        assert_eq!(
            <LineHeight as Parse>::parse_str("2"),
            Ok(LineHeight::Number(2.0)),
        );

        assert_eq!(
            <LineHeight as Parse>::parse_str("2cm"),
            Ok(LineHeight::Length(Length::new(2.0, LengthUnit::Cm))),
        );

        assert_eq!(
            <LineHeight as Parse>::parse_str("150%"),
            Ok(LineHeight::Percentage(1.5)),
        );
    }

    #[test]
    fn detects_invalid_line_height() {
        assert!(<LineHeight as Parse>::parse_str("").is_err());
        assert!(<LineHeight as Parse>::parse_str("florp").is_err());
        assert!(<LineHeight as Parse>::parse_str("3florp").is_err());
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
            LineHeight::Normal.compute(&values),
            LineHeight::Length(Length::new(10.0, LengthUnit::Px)),
        );

        assert_eq!(
            LineHeight::Number(2.0).compute(&values),
            LineHeight::Length(Length::new(20.0, LengthUnit::Px)),
        );

        assert_eq!(
            LineHeight::Length(Length::new(3.0, LengthUnit::Cm)).compute(&values),
            LineHeight::Length(Length::new(3.0, LengthUnit::Cm)),
        );

        assert_eq!(
            LineHeight::parse_str("150%").unwrap().compute(&values),
            LineHeight::Length(Length::new(15.0, LengthUnit::Px)),
        );
    }
}
