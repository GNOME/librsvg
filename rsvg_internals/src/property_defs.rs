use cssparser::{self, Parser, Token};

use crate::error::*;
use crate::font_props::{FontSizeSpec, FontWeightSpec, LetterSpacingSpec, SingleFontFamily};
use crate::iri::IRI;
use crate::length::*;
use crate::paint_server::PaintServer;
use crate::parsers::{Parse, ParseError};
use crate::properties::ComputedValues;
use crate::property_macros::Property;
use crate::unit_interval::UnitInterval;

// https://www.w3.org/TR/SVG/text.html#BaselineShiftProperty
make_property!(
    ComputedValues,
    BaselineShift,
    default: Length::<Both>::parse_str("0.0").unwrap(),
    newtype: Length<Both>,
    property_impl: {
        impl Property<ComputedValues> for BaselineShift {
            fn inherits_automatically() -> bool {
                false
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                let font_size = v.font_size.0.value();

                // FIXME: this implementation has limitations:
                // 1) we only handle 'percent' shifts, but it could also be an absolute offset
                // 2) we should be able to normalize the lengths and add even if they have
                //    different units, but at the moment that requires access to the draw_ctx
                if self.0.unit != LengthUnit::Percent || v.baseline_shift.0.unit != font_size.unit {
                    return BaselineShift(Length::<Both>::new(v.baseline_shift.0.length, v.baseline_shift.0.unit));
                }

                BaselineShift(Length::<Both>::new(self.0.length * font_size.length + v.baseline_shift.0.length, font_size.unit))
            }
        }
    },
    parse_impl: {
        impl Parse for BaselineShift {
            type Err = ValueErrorKind;

            // These values come from Inkscape's SP_CSS_BASELINE_SHIFT_(SUB/SUPER/BASELINE);
            // see sp_style_merge_baseline_shift_from_parent()
            fn parse(parser: &mut Parser<'_, '_>) -> Result<BaselineShift, crate::error::ValueErrorKind> {
                let parser_state = parser.state();

                {
                    let token = parser.next().map_err(|_| crate::error::ValueErrorKind::Parse(
                        crate::parsers::ParseError::new("expected token"),
                    ))?;

                    if let Token::Ident(ref cow) = token {
                        match cow.as_ref() {
                            "baseline" => return Ok(BaselineShift(
                                Length::<Both>::new(0.0, LengthUnit::Percent)
                            )),

                            "sub" => return Ok(BaselineShift(
                                Length::<Both>::new(-0.2, LengthUnit::Percent)
                            )),

                            "super" => return Ok(BaselineShift(
                                Length::<Both>::new(0.4, LengthUnit::Percent),
                            )),

                            _ => (),
                        }
                    }
                }

                parser.reset(&parser_state);

                Ok(BaselineShift(Length::<Both>::parse(parser)?))
            }
        }
    }
);

// https://www.w3.org/TR/SVG/masking.html#ClipPathProperty
make_property!(
    ComputedValues,
    ClipPath,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/masking.html#ClipRuleProperty
make_property!(
    ComputedValues,
    ClipRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

// https://www.w3.org/TR/SVG/color.html#ColorProperty
make_property!(
    ComputedValues,
    Color,
    // The SVG spec allows the user agent to choose its own default for the "color" property.
    // We don't allow passing in an initial CSS in the public API, so we'll start with black.
    //
    // See https://bugzilla.gnome.org/show_bug.cgi?id=764808 for a case where this would
    // be useful - rendering equations with currentColor, so they take on the color of the
    // surrounding text.
    default: cssparser::RGBA::new(0, 0, 0, 0xff),
    inherits_automatically: true,
    newtype_parse: cssparser::RGBA,
);

// https://www.w3.org/TR/SVG11/painting.html#ColorInterpolationProperty
make_property!(
    ComputedValues,
    ColorInterpolationFilters,
    default: LinearRgb,
    inherits_automatically: true,

    identifiers:
    "auto" => Auto,
    "linearRGB" => LinearRgb,
    "sRGB" => Srgb,
);

// https://www.w3.org/TR/SVG/text.html#DirectionProperty
make_property!(
    ComputedValues,
    Direction,
    default: Ltr,
    inherits_automatically: true,

    identifiers:
    "ltr" => Ltr,
    "rtl" => Rtl,
);

// https://www.w3.org/TR/SVG/painting.html#DisplayProperty
make_property!(
    ComputedValues,
    Display,
    default: Inline,
    inherits_automatically: false,

    identifiers:
    "inline" => Inline,
    "block" => Block,
    "list-item" => ListItem,
    "run-in" => RunIn,
    "compact" => Compact,
    "marker" => Marker,
    "table" => Table,
    "inline-table" => InlineTable,
    "table-row-group" => TableRowGroup,
    "table-header-group" => TableHeaderGroup,
    "table-footer-group" => TableFooterGroup,
    "table-row" => TableRow,
    "table-column-group" => TableColumnGroup,
    "table-column" => TableColumn,
    "table-cell" => TableCell,
    "table-caption" => TableCaption,
    "none" => None,
);

// https://www.w3.org/TR/SVG/filters.html#EnableBackgroundProperty
make_property!(
    ComputedValues,
    EnableBackground,
    default: Accumulate,
    inherits_automatically: false,

    identifiers:
    "accumulate" => Accumulate,
    "new" => New,
);

// https://www.w3.org/TR/SVG/painting.html#FillProperty
make_property!(
    ComputedValues,
    Fill,
    default: PaintServer::parse_str("#000").unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
);

// https://www.w3.org/TR/SVG/painting.html#FillOpacityProperty
make_property!(
    ComputedValues,
    FillOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/painting.html#FillRuleProperty
make_property!(
    ComputedValues,
    FillRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

// https://www.w3.org/TR/SVG/filters.html#FilterProperty
make_property!(
    ComputedValues,
    Filter,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/filters.html#FloodColorProperty
make_property!(
    ComputedValues,
    FloodColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
);

// https://www.w3.org/TR/SVG/filters.html#FloodOpacityProperty
make_property!(
    ComputedValues,
    FloodOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/text.html#FontFamilyProperty
make_property!(
    ComputedValues,
    FontFamily,
    default: SingleFontFamily("Times New Roman".to_string()),
    inherits_automatically: true,
    newtype_parse: SingleFontFamily,
);

// https://www.w3.org/TR/SVG/text.html#FontSizeProperty
make_property!(
    ComputedValues,
    FontSize,
    default: FontSizeSpec::Value(Length::<Both>::parse_str("12.0").unwrap()),
    newtype_parse: FontSizeSpec,
    property_impl: {
        impl Property<ComputedValues> for FontSize {
            fn inherits_automatically() -> bool {
                true
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                FontSize(self.0.compute(v))
            }
        }
    }
);

// https://www.w3.org/TR/SVG/text.html#FontStretchProperty
make_property!(
    ComputedValues,
    FontStretch,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "wider" => Wider,
    "narrower" => Narrower,
    "ultra-condensed" => UltraCondensed,
    "extra-condensed" => ExtraCondensed,
    "condensed" => Condensed,
    "semi-condensed" => SemiCondensed,
    "semi-expanded" => SemiExpanded,
    "expanded" => Expanded,
    "extra-expanded" => ExtraExpanded,
    "ultra-expanded" => UltraExpanded,
);

// https://www.w3.org/TR/SVG/text.html#FontStyleProperty
make_property!(
    ComputedValues,
    FontStyle,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "italic" => Italic,
    "oblique" => Oblique,
);

// https://www.w3.org/TR/SVG/text.html#FontVariantProperty
make_property!(
    ComputedValues,
    FontVariant,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "small-caps" => SmallCaps,
);

// https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-weight
make_property!(
    ComputedValues,
    FontWeight,
    default: FontWeightSpec::Normal,
    inherits_automatically: true,
    newtype_parse: FontWeightSpec,
);

// https://www.w3.org/TR/SVG/text.html#LetterSpacingProperty
make_property!(
    ComputedValues,
    LetterSpacing,
    default: LetterSpacingSpec::Normal,
    newtype_parse: LetterSpacingSpec,
    property_impl: {
        impl Property<ComputedValues> for LetterSpacing {
            fn inherits_automatically() -> bool {
                true
            }

            fn compute(&self, _v: &ComputedValues) -> Self {
                LetterSpacing(self.0.compute())
            }
        }
    }
);

// https://www.w3.org/TR/SVG/filters.html#LightingColorProperty
make_property!(
    ComputedValues,
    LightingColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(255, 255, 255, 255)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
);

make_property!(
    ComputedValues,
    Marker,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/painting.html#MarkerEndProperty
make_property!(
    ComputedValues,
    MarkerEnd,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/painting.html#MarkerMidProperty
make_property!(
    ComputedValues,
    MarkerMid,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/painting.html#MarkerStartProperty
make_property!(
    ComputedValues,
    MarkerStart,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/masking.html#MaskProperty
make_property!(
    ComputedValues,
    Mask,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
);

// https://www.w3.org/TR/SVG/masking.html#OpacityProperty
make_property!(
    ComputedValues,
    Opacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/masking.html#OverflowProperty
make_property!(
    ComputedValues,
    Overflow,
    default: Visible,
    inherits_automatically: false,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "scroll" => Scroll,
    "auto" => Auto,
);

// https://www.w3.org/TR/SVG/painting.html#ShapeRenderingProperty
make_property!(
    ComputedValues,
    ShapeRendering,
    default: Auto,
    inherits_automatically: true,

    identifiers:
    "auto" => Auto,
    "optimizeSpeed" => OptimizeSpeed,
    "geometricPrecision" => GeometricPrecision,
    "crispEdges" => CrispEdges,
);

// https://www.w3.org/TR/SVG/pservers.html#StopColorProperty
make_property!(
    ComputedValues,
    StopColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 255)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
);

// https://www.w3.org/TR/SVG/pservers.html#StopOpacityProperty
make_property!(
    ComputedValues,
    StopOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeProperty
make_property!(
    ComputedValues,
    Stroke,
    default: PaintServer::None,
    inherits_automatically: true,
    newtype_parse: PaintServer,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeDasharrayProperty
make_property!(
    ComputedValues,
    StrokeDasharray,
    default: Dasharray::default(),
    inherits_automatically: true,
    newtype_parse: Dasharray,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeDashoffsetProperty
make_property!(
    ComputedValues,
    StrokeDashoffset,
    default: Length::<Both>::default(),
    inherits_automatically: true,
    newtype_parse: Length<Both>,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeLinecapProperty
make_property!(
    ComputedValues,
    StrokeLinecap,
    default: Butt,
    inherits_automatically: true,

    identifiers:
    "butt" => Butt,
    "round" => Round,
    "square" => Square,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeLinejoinProperty
make_property!(
    ComputedValues,
    StrokeLinejoin,
    default: Miter,
    inherits_automatically: true,

    identifiers:
    "miter" => Miter,
    "round" => Round,
    "bevel" => Bevel,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeMiterlimitProperty
make_property!(
    ComputedValues,
    StrokeMiterlimit,
    default: 4f64,
    inherits_automatically: true,
    newtype_parse: f64,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeOpacityProperty
make_property!(
    ComputedValues,
    StrokeOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeWidthProperty
make_property!(
    ComputedValues,
    StrokeWidth,
    default: Length::<Both>::parse_str("1.0").unwrap(),
    inherits_automatically: true,
    newtype_parse: Length::<Both>,
);

// https://www.w3.org/TR/SVG/text.html#TextAnchorProperty
make_property!(
    ComputedValues,
    TextAnchor,
    default: Start,
    inherits_automatically: true,

    identifiers:
    "start" => Start,
    "middle" => Middle,
    "end" => End,
);

// https://www.w3.org/TR/SVG/text.html#TextDecorationProperty
make_property!(
    ComputedValues,
    TextDecoration,
    inherits_automatically: false,

    fields: {
        overline: bool, default: false,
        underline: bool, default: false,
        strike: bool, default: false,
    }

    parse_impl: {
        impl Parse for TextDecoration {
            type Err = ValueErrorKind;

            fn parse(parser: &mut Parser<'_, '_>) -> Result<TextDecoration, ValueErrorKind> {
                let mut overline = false;
                let mut underline = false;
                let mut strike = false;

                if parser.try_parse(|p| p.expect_ident_matching("none")).is_ok() {
                    return Ok(TextDecoration::default());
                }

                while !parser.is_exhausted() {
                    let cow = parser.expect_ident().map_err(|_| crate::error::ValueErrorKind::Parse(
                        crate::parsers::ParseError::new("expected identifier"),
                    ))?;

                    match cow.as_ref() {
                        "overline" => overline = true,
                        "underline" => underline = true,
                        "line-through" => strike = true,
                        _ => return Err(ValueErrorKind::Parse(ParseError::new("invalid syntax"))),
                    }
                }

                Ok(TextDecoration {
                    overline,
                    underline,
                    strike,
                })
            }
        }
    }
);

#[cfg(test)]
#[test]
fn parses_text_decoration() {
    assert_eq!(
        TextDecoration::parse_str("none").unwrap(),
        TextDecoration {
            overline: false,
            underline: false,
            strike: false,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("overline").unwrap(),
        TextDecoration {
            overline: true,
            underline: false,
            strike: false,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("underline").unwrap(),
        TextDecoration {
            overline: false,
            underline: true,
            strike: false,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("line-through").unwrap(),
        TextDecoration {
            overline: false,
            underline: false,
            strike: true,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("underline overline").unwrap(),
        TextDecoration {
            overline: true,
            underline: true,
            strike: false,
        }
    );

    assert!(TextDecoration::parse_str("airline").is_err())
}

// https://www.w3.org/TR/SVG/painting.html#TextRenderingProperty
make_property!(
    ComputedValues,
    TextRendering,
    default: Auto,
    inherits_automatically: true,

    identifiers:
    "auto" => Auto,
    "optimizeSpeed" => OptimizeSpeed,
    "optimizeLegibility" => OptimizeLegibility,
    "geometricPrecision" => GeometricPrecision,
);

// https://www.w3.org/TR/SVG/text.html#UnicodeBidiProperty
make_property!(
    ComputedValues,
    UnicodeBidi,
    default: Normal,
    inherits_automatically: false,

    identifiers:
    "normal" => Normal,
    "embed" => Embed,
    "bidi-override" => Override,
);

// https://www.w3.org/TR/SVG/painting.html#VisibilityProperty
make_property!(
    ComputedValues,
    Visibility,
    default: Visible,
    inherits_automatically: true,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "collapse" => Collapse,
);

// https://www.w3.org/TR/SVG/text.html#WritingModeProperty
make_property!(
    ComputedValues,
    WritingMode,
    default: LrTb,
    inherits_automatically: true,

    identifiers:
    "lr" => Lr,
    "lr-tb" => LrTb,
    "rl" => Rl,
    "rl-tb" => RlTb,
    "tb" => Tb,
    "tb-rl" => TbRl,
);

impl WritingMode {
    pub fn is_vertical(self) -> bool {
        match self {
            WritingMode::Tb | WritingMode::TbRl => true,
            _ => false,
        }
    }
}

make_property!(
    ComputedValues,
    XmlLang,
    default: "".to_string(), // see create_pango_layout()
    inherits_automatically: true,
    newtype: String,
    parse_impl: {
        impl Parse for XmlLang {
            type Err = ValueErrorKind;

            fn parse(
                parser: &mut Parser<'_, '_>,
            ) -> Result<XmlLang, ValueErrorKind> {
                Ok(XmlLang(parser.expect_ident()?.to_string()))
            }
        }
    },
);

#[cfg(test)]
#[test]
fn parses_xml_lang() {
    assert_eq!(
        XmlLang::parse_str("es-MX").unwrap(),
        XmlLang("es-MX".to_string())
    );

    assert!(XmlLang::parse_str("").is_err());
}

make_property!(
    ComputedValues,
    XmlSpace,
    default: Default,
    inherits_automatically: true,

    identifiers:
    "default" => Default,
    "preserve" => Preserve,
);
