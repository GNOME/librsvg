//! Definitions for CSS property types.
//!
//! Do not import things directly from this module; use the `properties` module instead,
//! which re-exports things from here.
//!
//! This module defines most of the CSS property types that librsvg supports.  Each
//! property requires a Rust type that will hold its values, and that type should
//! implement a few traits, as follows.
//!
//! # Requirements for a property type
//!
//! You should call the [`make_property`] macro to take care of most of these requirements
//! automatically:
//!
//! * A name for the type.  For example, the `fill` property has a [`Fill`] type defined
//! in this module.
//!
//! * An initial value per the CSS or SVG specs, given through an implementation of the
//! `Default` trait.
//!
//! * Whether the property's computed value inherits to child elements, given
//! through an implementation of the [`inherits_automatically`] method of the [`Property`]
//! trait.
//!
//! * A way to derive the CSS *computed value* for the property, given through an
//! implementation of the [`compute`] method of the [`Property`] trait.
//!
//! * The actual underlying type.  For example, the [`make_property`] macro can generate a
//! field-less enum for properties like the `clip-rule` property, which just has
//! identifier-based values like `nonzero` and `evenodd`.  For general-purpose types like
//! [`Length`], the macro can wrap them in a newtype like `struct`
//! [`StrokeWidth`]`(Length)`.  For custom types, the macro call can be used just to
//! define the initial/default value and whether the property inherits automatically; you
//! should provide the other required trait implementations separately.
//!
//! * An implementation of the [`Parse`] trait for the underlying type.
//!
//! [`compute`]: ../property_macros/trait.Property.html#tymethod.compute
//! [`inherits_automatically`]: ../property_macros/trait.Property.html#tymethod.inherits_automatically
//! [`Fill`]: struct.Fill.html
//! [`Length`]: ../length/struct.Length.html
//! [`make_property`]: ../macro.make_property.html
//! [`Opacity`]: struct.Opacity.html
//! [`Parse`]: ../trait.Parse.html
//! [`Property`]: ../property_macros/trait.Property.html
//! [`UnitInterval`]: ../unit_interval/struct.UnitInterval.html
use std::convert::TryInto;

use cssparser::{Parser, Token};

use crate::dasharray::Dasharray;
use crate::error::*;
use crate::filter::FilterValueList;
use crate::font_props::{Font, FontFamily, FontSize, FontWeight, LetterSpacing, LineHeight};
use crate::iri::Iri;
use crate::length::*;
use crate::paint_server::PaintServer;
use crate::parsers::Parse;
use crate::properties::ComputedValues;
use crate::property_macros::Property;
use crate::rect::Rect;
use crate::unit_interval::UnitInterval;

// https://www.w3.org/TR/SVG/text.html#BaselineShiftProperty
make_property!(
    BaselineShift,
    default: Length::<Both>::parse_str("0.0").unwrap(),
    newtype: Length<Both>,
    property_impl: {
        impl Property<ComputedValues> for BaselineShift {
            fn inherits_automatically() -> bool {
                false
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                let font_size = v.font_size().value();
                let parent = v.baseline_shift();

                match (self.0.unit, parent.0.unit) {
                    (LengthUnit::Percent, _) => {
                        BaselineShift(Length::<Both>::new(self.0.length * font_size.length + parent.0.length, font_size.unit))
                    }

                    (x, y) if x == y || parent.0.length == 0.0 => {
                        BaselineShift(Length::<Both>::new(self.0.length + parent.0.length, self.0.unit))
                    }

                    _ => {
                        // FIXME: the limitation here is that the parent's baseline_shift
                        // and ours have different units.  We should be able to normalize
                        // the lengths and add them even if they have different units, but
                        // at the moment that requires access to the draw_ctx, which we
                        // don't have here.
                        //
                        // So for now we won't add to the parent's baseline_shift.

                        parent
                    }
                }
            }
        }
    },
    parse_impl: {
        impl Parse for BaselineShift {
            // These values come from Inkscape's SP_CSS_BASELINE_SHIFT_(SUB/SUPER/BASELINE);
            // see sp_style_merge_baseline_shift_from_parent()
            fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<BaselineShift, crate::error::ParseError<'i>> {
                parser.try_parse(|p| Ok(BaselineShift(Length::<Both>::parse(p)?)))
                    .or_else(|_: ParseError<'_>| {
                        Ok(parse_identifiers!(
                            parser,
                            "baseline" => BaselineShift(Length::<Both>::new(0.0, LengthUnit::Percent)),
                            "sub" => BaselineShift(Length::<Both>::new(-0.2, LengthUnit::Percent)),

                            "super" => BaselineShift(Length::<Both>::new(0.4, LengthUnit::Percent)),
                        )?)
                    })
            }
        }
    }
);

// https://www.w3.org/TR/SVG/masking.html#ClipPathProperty
make_property!(
    ClipPath,
    default: Iri::None,
    inherits_automatically: false,
    newtype_parse: Iri,
);

// https://www.w3.org/TR/SVG/masking.html#ClipRuleProperty
make_property!(
    ClipRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

// https://www.w3.org/TR/SVG/color.html#ColorProperty
make_property!(
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
    Direction,
    default: Ltr,
    inherits_automatically: true,

    identifiers:
    "ltr" => Ltr,
    "rtl" => Rtl,
);

// https://www.w3.org/TR/CSS2/visuren.html#display-prop
make_property!(
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnableBackground {
    Accumulate,
    New(Option<Rect>),
}

// https://www.w3.org/TR/SVG/filters.html#EnableBackgroundProperty
make_property!(
    EnableBackground,
    default: EnableBackground::Accumulate,
    inherits_automatically: false,

    parse_impl: {
        impl Parse for EnableBackground {
            fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, crate::error::ParseError<'i>> {
                let loc = parser.current_source_location();

                if parser
                    .try_parse(|p| p.expect_ident_matching("accumulate"))
                    .is_ok()
                {
                    return Ok(EnableBackground::Accumulate);
                }

                if parser.try_parse(|p| p.expect_ident_matching("new")).is_ok() {
                    parser.try_parse(|p| -> Result<_, ParseError<'_>> {
                        let x = f64::parse(p)?;
                        let y = f64::parse(p)?;
                        let w = f64::parse(p)?;
                        let h = f64::parse(p)?;

                        Ok(EnableBackground::New(Some(Rect::new(x, y, x + w, y + h))))
                    }).or(Ok(EnableBackground::New(None)))
                } else {
                    Err(loc.new_custom_error(ValueErrorKind::parse_error("invalid syntax for 'enable-background' property")))
                }
            }
        }

    }
);

#[cfg(test)]
#[test]
fn parses_enable_background() {
    assert_eq!(
        EnableBackground::parse_str("accumulate").unwrap(),
        EnableBackground::Accumulate
    );

    assert_eq!(
        EnableBackground::parse_str("new").unwrap(),
        EnableBackground::New(None)
    );

    assert_eq!(
        EnableBackground::parse_str("new 1 2 3 4").unwrap(),
        EnableBackground::New(Some(Rect::new(1.0, 2.0, 4.0, 6.0)))
    );

    assert!(EnableBackground::parse_str("new foo").is_err());

    assert!(EnableBackground::parse_str("plonk").is_err());
}

// https://www.w3.org/TR/SVG/painting.html#FillProperty
make_property!(
    Fill,
    default: PaintServer::parse_str("#000").unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
);

// https://www.w3.org/TR/SVG/painting.html#FillOpacityProperty
make_property!(
    FillOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/painting.html#FillRuleProperty
make_property!(
    FillRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    None,
    List(FilterValueList),
}
// https://www.w3.org/TR/SVG/filters.html#FilterProperty
make_property!(
    Filter,
    default: Filter::None,
    inherits_automatically: false,
    parse_impl: {
        impl Parse for Filter {
            fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, crate::error::ParseError<'i>> {

                if parser
                    .try_parse(|p| p.expect_ident_matching("none"))
                    .is_ok()
                {
                    return Ok(Filter::None);
                }

                Ok(Filter::List(FilterValueList::parse(parser)?))
            }
        }
    }
);

// https://www.w3.org/TR/SVG/filters.html#FloodColorProperty
make_property!(
    FloodColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
);

// https://www.w3.org/TR/SVG/filters.html#FloodOpacityProperty
make_property!(
    FloodOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
);

// https://drafts.csswg.org/css-fonts-4/#font-prop
make_property!(
    Font,
    default: Font::Spec(Default::default()),
    inherits_automatically: true,
);

// https://www.w3.org/TR/SVG/text.html#FontFamilyProperty
make_property!(
    FontFamily,
    default: FontFamily("Times New Roman".to_string()),
    inherits_automatically: true,
);

// https://www.w3.org/TR/SVG/text.html#FontSizeProperty
make_property!(
    FontSize,
    default: FontSize::Value(Length::<Both>::parse_str("12.0").unwrap()),
    property_impl: {
        impl Property<ComputedValues> for FontSize {
            fn inherits_automatically() -> bool {
                true
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                self.compute(v)
            }
        }
    }
);

// https://www.w3.org/TR/SVG/text.html#FontStretchProperty
make_property!(
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
    FontVariant,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "small-caps" => SmallCaps,
);

// https://drafts.csswg.org/css-fonts-4/#font-weight-prop
make_property!(
    FontWeight,
    default: FontWeight::Normal,
    property_impl: {
        impl Property<ComputedValues> for FontWeight {
            fn inherits_automatically() -> bool {
                true
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                self.compute(&v.font_weight())
            }
        }
    }
);

// https://www.w3.org/TR/SVG/text.html#LetterSpacingProperty
make_property!(
    LetterSpacing,
    default: LetterSpacing::Normal,
    property_impl: {
        impl Property<ComputedValues> for LetterSpacing {
            fn inherits_automatically() -> bool {
                true
            }

            fn compute(&self, _v: &ComputedValues) -> Self {
                self.compute()
            }
        }
    }
);

// https://drafts.csswg.org/css2/visudet.html#propdef-line-height
make_property!(
    LineHeight,
    default: LineHeight::Normal,
    inherits_automatically: true,
);

// https://www.w3.org/TR/SVG/filters.html#LightingColorProperty
make_property!(
    LightingColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(255, 255, 255, 255)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
);

make_property!(
    Marker,
    default: Iri::None,
    inherits_automatically: true,
    newtype_parse: Iri,
);

// https://www.w3.org/TR/SVG/painting.html#MarkerEndProperty
make_property!(
    MarkerEnd,
    default: Iri::None,
    inherits_automatically: true,
    newtype_parse: Iri,
);

// https://www.w3.org/TR/SVG/painting.html#MarkerMidProperty
make_property!(
    MarkerMid,
    default: Iri::None,
    inherits_automatically: true,
    newtype_parse: Iri,
);

// https://www.w3.org/TR/SVG/painting.html#MarkerStartProperty
make_property!(
    MarkerStart,
    default: Iri::None,
    inherits_automatically: true,
    newtype_parse: Iri,
);

// https://www.w3.org/TR/SVG/masking.html#MaskProperty
make_property!(
    Mask,
    default: Iri::None,
    inherits_automatically: false,
    newtype_parse: Iri,
);

// https://www.w3.org/TR/compositing/#mix-blend-mode
make_property!(
    MixBlendMode,
    default: Normal,
    inherits_automatically: false,

    identifiers:
    "normal" => Normal,
    "multiply" => Multiply,
    "screen" => Screen,
    "overlay" => Overlay,
    "darken" => Darken,
    "lighten" => Lighten,
    "color-dodge" => ColorDodge,
    "color-burn" => ColorBurn,
    "hard-light" => HardLight,
    "soft-light" => SoftLight,
    "difference" => Difference,
    "exclusion" => Exclusion,
    "hue" => Hue,
    "saturation" => Saturation,
    "color" => Color,
    "luminosity" => Luminosity,
);

// https://www.w3.org/TR/SVG/masking.html#OpacityProperty
make_property!(
    Opacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/masking.html#OverflowProperty
make_property!(
    Overflow,
    default: Visible,
    inherits_automatically: false,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "scroll" => Scroll,
    "auto" => Auto,
);

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PaintTarget {
    Fill,
    Stroke,
    Markers,
}

// https://www.w3.org/TR/SVG2/painting.html#PaintOrder
make_property!(
    PaintOrder,
    inherits_automatically: true,
    fields: {
        targets: [PaintTarget; 3], default: [PaintTarget::Fill, PaintTarget::Stroke, PaintTarget::Markers],
    }

    parse_impl: {
        impl Parse for PaintOrder {
            fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<PaintOrder, ParseError<'i>> {
                let allowed_targets = 3;
                let mut targets = Vec::with_capacity(allowed_targets);

                if parser.try_parse(|p| p.expect_ident_matching("normal")).is_ok() {
                    return Ok(PaintOrder::default());
                }

                while !parser.is_exhausted() {
                    let loc = parser.current_source_location();
                    let token = parser.next()?;

                    let value = match token {
                        Token::Ident(ref cow) if cow.eq_ignore_ascii_case("fill") && !targets.contains(&PaintTarget::Fill) => PaintTarget::Fill,
                        Token::Ident(ref cow) if cow.eq_ignore_ascii_case("stroke") && !targets.contains(&PaintTarget::Stroke) => PaintTarget::Stroke,
                        Token::Ident(ref cow) if cow.eq_ignore_ascii_case("markers") && !targets.contains(&PaintTarget::Markers) => PaintTarget::Markers,
                        _ => return Err(loc.new_basic_unexpected_token_error(token.clone()).into()),
                    };

                    targets.push(value);
                };

                // any values which were not specfied should be painted in default order
                // (fill, stroke, markers) following the values which were explicitly specified.
                for &target in &[PaintTarget::Fill, PaintTarget::Stroke, PaintTarget::Markers] {
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                }
                Ok(PaintOrder {
                    targets: targets[..].try_into().expect("Incorrect number of targets in paint-order")
                })
            }
        }
    }
);

#[cfg(test)]
#[test]
fn parses_paint_order() {
    assert_eq!(
        PaintOrder::parse_str("normal").unwrap(),
        PaintOrder {
            targets: [PaintTarget::Fill, PaintTarget::Stroke, PaintTarget::Markers]
        }
    );

    assert_eq!(
        PaintOrder::parse_str("markers fill").unwrap(),
        PaintOrder {
            targets: [PaintTarget::Markers, PaintTarget::Fill, PaintTarget::Stroke]
        }
    );

    assert_eq!(
        PaintOrder::parse_str("stroke").unwrap(),
        PaintOrder {
            targets: [PaintTarget::Stroke, PaintTarget::Fill, PaintTarget::Markers]
        }
    );

    assert!(PaintOrder::parse_str("stroke stroke").is_err());
    assert!(PaintOrder::parse_str("markers stroke fill hello").is_err());
}

// https://www.w3.org/TR/SVG/painting.html#ShapeRenderingProperty
make_property!(
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
    StopColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 255)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
);

// https://www.w3.org/TR/SVG/pservers.html#StopOpacityProperty
make_property!(
    StopOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeProperty
make_property!(
    Stroke,
    default: PaintServer::None,
    inherits_automatically: true,
    newtype_parse: PaintServer,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeDasharrayProperty
make_property!(
    StrokeDasharray,
    default: Dasharray::default(),
    inherits_automatically: true,
    newtype_parse: Dasharray,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeDashoffsetProperty
make_property!(
    StrokeDashoffset,
    default: Length::<Both>::default(),
    inherits_automatically: true,
    newtype_parse: Length<Both>,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeLinecapProperty
make_property!(
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
    StrokeMiterlimit,
    default: 4f64,
    inherits_automatically: true,
    newtype_parse: f64,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeOpacityProperty
make_property!(
    StrokeOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_parse: UnitInterval,
);

// https://www.w3.org/TR/SVG/painting.html#StrokeWidthProperty
make_property!(
    StrokeWidth,
    default: Length::<Both>::parse_str("1.0").unwrap(),
    inherits_automatically: true,
    newtype_parse: Length::<Both>,
);

// https://www.w3.org/TR/SVG/text.html#TextAnchorProperty
make_property!(
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
    TextDecoration,
    inherits_automatically: false,

    fields: {
        overline: bool, default: false,
        underline: bool, default: false,
        strike: bool, default: false,
    }

    parse_impl: {
        impl Parse for TextDecoration {
            fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<TextDecoration, ParseError<'i>> {
                let mut overline = false;
                let mut underline = false;
                let mut strike = false;

                if parser.try_parse(|p| p.expect_ident_matching("none")).is_ok() {
                    return Ok(TextDecoration::default());
                }

                while !parser.is_exhausted() {
                    let loc = parser.current_source_location();
                    let token = parser.next()?;

                    match token {
                        Token::Ident(ref cow) if cow.eq_ignore_ascii_case("overline") => overline = true,
                        Token::Ident(ref cow) if cow.eq_ignore_ascii_case("underline") => underline = true,
                        Token::Ident(ref cow) if cow.eq_ignore_ascii_case("line-through") => strike = true,
                        _ => return Err(loc.new_basic_unexpected_token_error(token.clone()).into()),
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
    UnicodeBidi,
    default: Normal,
    inherits_automatically: false,

    identifiers:
    "normal" => Normal,
    "embed" => Embed,
    "bidi-override" => Override,
);

// https://www.w3.org/TR/CSS2/visufx.html#visibility
make_property!(
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
        matches!(self, WritingMode::Tb | WritingMode::TbRl)
    }
}

make_property!(
    XmlLang,
    default: None,
    inherits_automatically: true,
    newtype: Option<String>,
    parse_impl: {
        impl Parse for XmlLang {
            fn parse<'i>(
                parser: &mut Parser<'i, '_>,
            ) -> Result<XmlLang, ParseError<'i>> {
                Ok(XmlLang(Some(parser.expect_ident()?.to_string())))
            }
        }
    },
);

#[cfg(test)]
#[test]
fn parses_xml_lang() {
    assert_eq!(
        XmlLang::parse_str("es-MX").unwrap(),
        XmlLang(Some("es-MX".to_string()))
    );

    assert!(XmlLang::parse_str("").is_err());
}

make_property!(
    XmlSpace,
    default: Default,
    inherits_automatically: true,

    identifiers:
    "default" => Default,
    "preserve" => Preserve,
);
