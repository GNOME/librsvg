//! CSS properties, specified values, computed values.

use cssparser::{
    self, BasicParseErrorKind, DeclarationListParser, ParseErrorKind, Parser, ParserInput, ToCss,
};
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use std::collections::HashSet;

use crate::css::{DeclParser, Declaration};
use crate::error::*;
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::property_defs::*;
use crate::property_macros::Property;

/// Representation of a single CSS property value.
///
/// `Unspecified` is the `Default`; it means that the corresponding property is not present.
///
/// `Inherit` means that the property is explicitly set to inherit
/// from the parent element.  This is useful for properties which the
/// SVG or CSS specs mandate that should not be inherited by default.
///
/// `Specified` is a value given by the SVG or CSS stylesheet.  This will later be
/// resolved into part of a `ComputedValues` struct.
#[derive(Clone)]
pub enum SpecifiedValue<T>
where
    T: Property<ComputedValues> + Clone + Default,
{
    Unspecified,
    Inherit,
    Specified(T),
}

impl<T> SpecifiedValue<T>
where
    T: Property<ComputedValues> + Clone + Default,
{
    pub fn compute(&self, src: &T, src_values: &ComputedValues) -> T {
        let value: T = match *self {
            SpecifiedValue::Unspecified => {
                if <T as Property<ComputedValues>>::inherits_automatically() {
                    src.clone()
                } else {
                    Default::default()
                }
            }

            SpecifiedValue::Inherit => src.clone(),

            SpecifiedValue::Specified(ref v) => v.clone(),
        };

        value.compute(src_values)
    }
}

impl<T> Default for SpecifiedValue<T>
where
    T: Property<ComputedValues> + Clone + Default,
{
    fn default() -> SpecifiedValue<T> {
        SpecifiedValue::Unspecified
    }
}

/// Embodies "which property is this" plus the property's value
pub enum ParsedProperty {
    BaselineShift(SpecifiedValue<BaselineShift>),
    ClipPath(SpecifiedValue<ClipPath>),
    ClipRule(SpecifiedValue<ClipRule>),
    Color(SpecifiedValue<Color>),
    ColorInterpolationFilters(SpecifiedValue<ColorInterpolationFilters>),
    Direction(SpecifiedValue<Direction>),
    Display(SpecifiedValue<Display>),
    EnableBackground(SpecifiedValue<EnableBackground>),
    Fill(SpecifiedValue<Fill>),
    FillOpacity(SpecifiedValue<FillOpacity>),
    FillRule(SpecifiedValue<FillRule>),
    Filter(SpecifiedValue<Filter>),
    FloodColor(SpecifiedValue<FloodColor>),
    FloodOpacity(SpecifiedValue<FloodOpacity>),
    FontFamily(SpecifiedValue<FontFamily>),
    FontSize(SpecifiedValue<FontSize>),
    FontStretch(SpecifiedValue<FontStretch>),
    FontStyle(SpecifiedValue<FontStyle>),
    FontVariant(SpecifiedValue<FontVariant>),
    FontWeight(SpecifiedValue<FontWeight>),
    LetterSpacing(SpecifiedValue<LetterSpacing>),
    LightingColor(SpecifiedValue<LightingColor>),
    Marker(SpecifiedValue<Marker>), // this is a shorthand property
    MarkerEnd(SpecifiedValue<MarkerEnd>),
    MarkerMid(SpecifiedValue<MarkerMid>),
    MarkerStart(SpecifiedValue<MarkerStart>),
    Mask(SpecifiedValue<Mask>),
    Opacity(SpecifiedValue<Opacity>),
    Overflow(SpecifiedValue<Overflow>),
    ShapeRendering(SpecifiedValue<ShapeRendering>),
    StopColor(SpecifiedValue<StopColor>),
    StopOpacity(SpecifiedValue<StopOpacity>),
    Stroke(SpecifiedValue<Stroke>),
    StrokeDasharray(SpecifiedValue<StrokeDasharray>),
    StrokeDashoffset(SpecifiedValue<StrokeDashoffset>),
    StrokeLinecap(SpecifiedValue<StrokeLinecap>),
    StrokeLinejoin(SpecifiedValue<StrokeLinejoin>),
    StrokeOpacity(SpecifiedValue<StrokeOpacity>),
    StrokeMiterlimit(SpecifiedValue<StrokeMiterlimit>),
    StrokeWidth(SpecifiedValue<StrokeWidth>),
    TextAnchor(SpecifiedValue<TextAnchor>),
    TextDecoration(SpecifiedValue<TextDecoration>),
    TextRendering(SpecifiedValue<TextRendering>),
    UnicodeBidi(SpecifiedValue<UnicodeBidi>),
    Visibility(SpecifiedValue<Visibility>),
    WritingMode(SpecifiedValue<WritingMode>),
}

/// Holds the specified CSS properties
///
/// This is used for various purposes:
///
/// * Immutably, to store the attributes of element nodes after parsing.
/// * Mutably, during cascading/rendering.
///
/// Each property should have its own data type, and implement
/// `Default` and `parsers::Parse`.
#[derive(Default, Clone)]
pub struct SpecifiedValues {
    pub baseline_shift: SpecifiedValue<BaselineShift>,
    pub clip_path: SpecifiedValue<ClipPath>,
    pub clip_rule: SpecifiedValue<ClipRule>,
    pub color: SpecifiedValue<Color>,
    pub color_interpolation_filters: SpecifiedValue<ColorInterpolationFilters>,
    pub direction: SpecifiedValue<Direction>,
    pub display: SpecifiedValue<Display>,
    pub enable_background: SpecifiedValue<EnableBackground>,
    pub fill: SpecifiedValue<Fill>,
    pub fill_opacity: SpecifiedValue<FillOpacity>,
    pub fill_rule: SpecifiedValue<FillRule>,
    pub filter: SpecifiedValue<Filter>,
    pub flood_color: SpecifiedValue<FloodColor>,
    pub flood_opacity: SpecifiedValue<FloodOpacity>,
    pub font_family: SpecifiedValue<FontFamily>,
    pub font_size: SpecifiedValue<FontSize>,
    pub font_stretch: SpecifiedValue<FontStretch>,
    pub font_style: SpecifiedValue<FontStyle>,
    pub font_variant: SpecifiedValue<FontVariant>,
    pub font_weight: SpecifiedValue<FontWeight>,
    pub letter_spacing: SpecifiedValue<LetterSpacing>,
    pub lighting_color: SpecifiedValue<LightingColor>,
    pub marker_end: SpecifiedValue<MarkerEnd>,
    pub marker_mid: SpecifiedValue<MarkerMid>,
    pub marker_start: SpecifiedValue<MarkerStart>,
    pub mask: SpecifiedValue<Mask>,
    pub opacity: SpecifiedValue<Opacity>,
    pub overflow: SpecifiedValue<Overflow>,
    pub shape_rendering: SpecifiedValue<ShapeRendering>,
    pub stop_color: SpecifiedValue<StopColor>,
    pub stop_opacity: SpecifiedValue<StopOpacity>,
    pub stroke: SpecifiedValue<Stroke>,
    pub stroke_dasharray: SpecifiedValue<StrokeDasharray>,
    pub stroke_dashoffset: SpecifiedValue<StrokeDashoffset>,
    pub stroke_line_cap: SpecifiedValue<StrokeLinecap>,
    pub stroke_line_join: SpecifiedValue<StrokeLinejoin>,
    pub stroke_opacity: SpecifiedValue<StrokeOpacity>,
    pub stroke_miterlimit: SpecifiedValue<StrokeMiterlimit>,
    pub stroke_width: SpecifiedValue<StrokeWidth>,
    pub text_anchor: SpecifiedValue<TextAnchor>,
    pub text_decoration: SpecifiedValue<TextDecoration>,
    pub text_rendering: SpecifiedValue<TextRendering>,
    pub unicode_bidi: SpecifiedValue<UnicodeBidi>,
    pub visibility: SpecifiedValue<Visibility>,
    pub writing_mode: SpecifiedValue<WritingMode>,
    pub xml_lang: SpecifiedValue<XmlLang>, // not a property, but a non-presentation attribute
    pub xml_space: SpecifiedValue<XmlSpace>, // not a property, but a non-presentation attribute
}

#[derive(Debug, Default, Clone)]
pub struct ComputedValues {
    pub baseline_shift: BaselineShift,
    pub clip_path: ClipPath,
    pub clip_rule: ClipRule,
    pub color: Color,
    pub color_interpolation_filters: ColorInterpolationFilters,
    pub direction: Direction,
    pub display: Display,
    pub enable_background: EnableBackground,
    pub fill: Fill,
    pub fill_opacity: FillOpacity,
    pub fill_rule: FillRule,
    pub filter: Filter,
    pub flood_color: FloodColor,
    pub flood_opacity: FloodOpacity,
    pub font_family: FontFamily,
    pub font_size: FontSize,
    pub font_stretch: FontStretch,
    pub font_style: FontStyle,
    pub font_variant: FontVariant,
    pub font_weight: FontWeight,
    pub letter_spacing: LetterSpacing,
    pub lighting_color: LightingColor,
    pub marker_end: MarkerEnd,
    pub marker_mid: MarkerMid,
    pub marker_start: MarkerStart,
    pub mask: Mask,
    pub opacity: Opacity,
    pub overflow: Overflow,
    pub shape_rendering: ShapeRendering,
    pub stop_color: StopColor,
    pub stop_opacity: StopOpacity,
    pub stroke: Stroke,
    pub stroke_dasharray: StrokeDasharray,
    pub stroke_dashoffset: StrokeDashoffset,
    pub stroke_line_cap: StrokeLinecap,
    pub stroke_line_join: StrokeLinejoin,
    pub stroke_opacity: StrokeOpacity,
    pub stroke_miterlimit: StrokeMiterlimit,
    pub stroke_width: StrokeWidth,
    pub text_anchor: TextAnchor,
    pub text_decoration: TextDecoration,
    pub text_rendering: TextRendering,
    pub unicode_bidi: UnicodeBidi,
    pub visibility: Visibility,
    pub writing_mode: WritingMode,
    pub xml_lang: XmlLang,   // not a property, but a non-presentation attribute
    pub xml_space: XmlSpace, // not a property, but a non-presentation attribute
}

#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn parse_property<'i>(prop_name: &QualName, input: &mut Parser<'i, '_>, accept_shorthands: bool) -> Result<ParsedProperty, CssParseError<'i>> {
    // please keep these sorted
    match prop_name.expanded() {
        expanded_name!("", "baseline-shift") =>
            Ok(ParsedProperty::BaselineShift(parse_input(input)?)),

        expanded_name!("", "clip-path") =>
            Ok(ParsedProperty::ClipPath(parse_input(input)?)),

        expanded_name!("", "clip-rule") =>
            Ok(ParsedProperty::ClipRule(parse_input(input)?)),

        expanded_name!("", "color") =>
            Ok(ParsedProperty::Color(parse_input(input)?)),

        expanded_name!("", "color-interpolation-filters") =>
            Ok(ParsedProperty::ColorInterpolationFilters(parse_input(input)?)),

        expanded_name!("", "direction") =>
            Ok(ParsedProperty::Direction(parse_input(input)?)),

        expanded_name!("", "display") =>
            Ok(ParsedProperty::Display(parse_input(input)?)),

        expanded_name!("", "enable-background") =>
            Ok(ParsedProperty::EnableBackground(parse_input(input)?)),

        expanded_name!("", "fill") =>
            Ok(ParsedProperty::Fill(parse_input(input)?)),

        expanded_name!("", "fill-opacity") =>
            Ok(ParsedProperty::FillOpacity(parse_input(input)?)),

        expanded_name!("", "fill-rule") =>
            Ok(ParsedProperty::FillRule(parse_input(input)?)),

        expanded_name!("", "filter") =>
            Ok(ParsedProperty::Filter(parse_input(input)?)),

        expanded_name!("", "flood-color") =>
            Ok(ParsedProperty::FloodColor(parse_input(input)?)),

        expanded_name!("", "flood-opacity") =>
            Ok(ParsedProperty::FloodOpacity(parse_input(input)?)),

        expanded_name!("", "font-family") =>
            Ok(ParsedProperty::FontFamily(parse_input(input)?)),

        expanded_name!("", "font-size") =>
            Ok(ParsedProperty::FontSize(parse_input(input)?)),

        expanded_name!("", "font-stretch") =>
            Ok(ParsedProperty::FontStretch(parse_input(input)?)),

        expanded_name!("", "font-style") =>
            Ok(ParsedProperty::FontStyle(parse_input(input)?)),

        expanded_name!("", "font-variant") =>
            Ok(ParsedProperty::FontVariant(parse_input(input)?)),

        expanded_name!("", "font-weight") =>
            Ok(ParsedProperty::FontWeight(parse_input(input)?)),

        expanded_name!("", "letter-spacing") =>
            Ok(ParsedProperty::LetterSpacing(parse_input(input)?)),

        expanded_name!("", "lighting-color") =>
            Ok(ParsedProperty::LightingColor(parse_input(input)?)),

        expanded_name!("", "marker") => {
            if accept_shorthands {
                Ok(ParsedProperty::Marker(parse_input(input)?))
            } else {
                let loc = input.current_source_location();
                Err(loc.new_custom_error(ValueErrorKind::UnknownProperty))
            }
        }

        expanded_name!("", "marker-end") =>
            Ok(ParsedProperty::MarkerEnd(parse_input(input)?)),

        expanded_name!("", "marker-mid") =>
            Ok(ParsedProperty::MarkerMid(parse_input(input)?)),

        expanded_name!("", "marker-start") =>
            Ok(ParsedProperty::MarkerStart(parse_input(input)?)),

        expanded_name!("", "mask") =>
            Ok(ParsedProperty::Mask(parse_input(input)?)),

        expanded_name!("", "opacity") =>
            Ok(ParsedProperty::Opacity(parse_input(input)?)),

        expanded_name!("", "overflow") =>
            Ok(ParsedProperty::Overflow(parse_input(input)?)),

        expanded_name!("", "shape-rendering") =>
            Ok(ParsedProperty::ShapeRendering(parse_input(input)?)),

        expanded_name!("", "stop-color") =>
            Ok(ParsedProperty::StopColor(parse_input(input)?)),

        expanded_name!("", "stop-opacity") =>
            Ok(ParsedProperty::StopOpacity(parse_input(input)?)),

        expanded_name!("", "stroke") =>
            Ok(ParsedProperty::Stroke(parse_input(input)?)),

        expanded_name!("", "stroke-dasharray") =>
            Ok(ParsedProperty::StrokeDasharray(parse_input(input)?)),

        expanded_name!("", "stroke-dashoffset") =>
            Ok(ParsedProperty::StrokeDashoffset(parse_input(input)?)),

        expanded_name!("", "stroke-linecap") =>
            Ok(ParsedProperty::StrokeLinecap(parse_input(input)?)),

        expanded_name!("", "stroke-linejoin") =>
            Ok(ParsedProperty::StrokeLinejoin(parse_input(input)?)),

        expanded_name!("", "stroke-miterlimit") =>
            Ok(ParsedProperty::StrokeMiterlimit(parse_input(input)?)),

        expanded_name!("", "stroke-opacity") =>
            Ok(ParsedProperty::StrokeOpacity(parse_input(input)?)),

        expanded_name!("", "stroke-width") =>
            Ok(ParsedProperty::StrokeWidth(parse_input(input)?)),

        expanded_name!("", "text-anchor") =>
            Ok(ParsedProperty::TextAnchor(parse_input(input)?)),

        expanded_name!("", "text-decoration") =>
            Ok(ParsedProperty::TextDecoration(parse_input(input)?)),

        expanded_name!("", "text-rendering") =>
            Ok(ParsedProperty::TextRendering(parse_input(input)?)),

        expanded_name!("", "unicode-bidi") =>
            Ok(ParsedProperty::UnicodeBidi(parse_input(input)?)),

        expanded_name!("", "visibility") =>
            Ok(ParsedProperty::Visibility(parse_input(input)?)),

        expanded_name!("", "writing-mode") =>
            Ok(ParsedProperty::WritingMode(parse_input(input)?)),

        _ => {
            let loc = input.current_source_location();
            Err(loc.new_custom_error(ValueErrorKind::UnknownProperty))
        }
    }
}

impl ComputedValues {
    pub fn is_overflow(&self) -> bool {
        match self.overflow {
            Overflow::Auto | Overflow::Visible => true,
            _ => false,
        }
    }

    pub fn is_visible(&self) -> bool {
        match (self.display, self.visibility) {
            (Display::None, _) => false,
            (_, Visibility::Visible) => true,
            _ => false,
        }
    }
}

macro_rules! compute_value {
    ($self:ident, $computed:ident, $name:ident) => {
        $computed.$name = $self.$name.compute(&$computed.$name, &$computed)
    };
}

impl SpecifiedValues {
    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub fn set_parsed_property(&mut self, prop: &ParsedProperty) {
        use crate::properties::ParsedProperty::*;

        use crate::properties as p;

        match *prop {
            BaselineShift(ref x)             => self.baseline_shift               = x.clone(),
            ClipPath(ref x)                  => self.clip_path                    = x.clone(),
            ClipRule(ref x)                  => self.clip_rule                    = x.clone(),
            Color(ref x)                     => self.color                        = x.clone(),
            ColorInterpolationFilters(ref x) => self.color_interpolation_filters  = x.clone(),
            Direction(ref x)                 => self.direction                    = x.clone(),
            Display(ref x)                   => self.display                      = x.clone(),
            EnableBackground(ref x)          => self.enable_background            = x.clone(),
            Fill(ref x)                      => self.fill                         = x.clone(),
            FillOpacity(ref x)               => self.fill_opacity                 = x.clone(),
            FillRule(ref x)                  => self.fill_rule                    = x.clone(),
            Filter(ref x)                    => self.filter                       = x.clone(),
            FloodColor(ref x)                => self.flood_color                  = x.clone(),
            FloodOpacity(ref x)              => self.flood_opacity                = x.clone(),
            FontFamily(ref x)                => self.font_family                  = x.clone(),
            FontSize(ref x)                  => self.font_size                    = x.clone(),
            FontStretch(ref x)               => self.font_stretch                 = x.clone(),
            FontStyle(ref x)                 => self.font_style                   = x.clone(),
            FontVariant(ref x)               => self.font_variant                 = x.clone(),
            FontWeight(ref x)                => self.font_weight                  = x.clone(),
            LetterSpacing(ref x)             => self.letter_spacing               = x.clone(),
            LightingColor(ref x)             => self.lighting_color               = x.clone(),

            Marker(ref x) => if let SpecifiedValue::Specified(p::Marker(ref v)) = *x {
                // Since "marker" is a shorthand property, we'll just expand it here
                self.marker_end = SpecifiedValue::Specified(p::MarkerEnd(v.clone()));
                self.marker_mid = SpecifiedValue::Specified(p::MarkerMid(v.clone()));
                self.marker_start = SpecifiedValue::Specified(p::MarkerStart(v.clone()));
            },

            MarkerEnd(ref x)                 => self.marker_end                   = x.clone(),
            MarkerMid(ref x)                 => self.marker_mid                   = x.clone(),
            MarkerStart(ref x)               => self.marker_start                 = x.clone(),
            Mask(ref x)                      => self.mask                         = x.clone(),
            Opacity(ref x)                   => self.opacity                      = x.clone(),
            Overflow(ref x)                  => self.overflow                     = x.clone(),
            ShapeRendering(ref x)            => self.shape_rendering              = x.clone(),
            StopColor(ref x)                 => self.stop_color                   = x.clone(),
            StopOpacity(ref x)               => self.stop_opacity                 = x.clone(),
            Stroke(ref x)                    => self.stroke                       = x.clone(),
            StrokeDasharray(ref x)           => self.stroke_dasharray             = x.clone(),
            StrokeDashoffset(ref x)          => self.stroke_dashoffset            = x.clone(),
            StrokeLinecap(ref x)             => self.stroke_line_cap              = x.clone(),
            StrokeLinejoin(ref x)            => self.stroke_line_join             = x.clone(),
            StrokeOpacity(ref x)             => self.stroke_opacity               = x.clone(),
            StrokeMiterlimit(ref x)          => self.stroke_miterlimit            = x.clone(),
            StrokeWidth(ref x)               => self.stroke_width                 = x.clone(),
            TextAnchor(ref x)                => self.text_anchor                  = x.clone(),
            TextDecoration(ref x)            => self.text_decoration              = x.clone(),
            TextRendering(ref x)             => self.text_rendering               = x.clone(),
            UnicodeBidi(ref x)               => self.unicode_bidi                 = x.clone(),
            Visibility(ref x)                => self.visibility                   = x.clone(),
            WritingMode(ref x)               => self.writing_mode                 = x.clone(),
        }
    }

    pub fn to_computed_values(&self, computed: &mut ComputedValues) {
        compute_value!(self, computed, baseline_shift);
        compute_value!(self, computed, clip_path);
        compute_value!(self, computed, clip_rule);
        compute_value!(self, computed, color);
        compute_value!(self, computed, color_interpolation_filters);
        compute_value!(self, computed, direction);
        compute_value!(self, computed, display);
        compute_value!(self, computed, enable_background);
        compute_value!(self, computed, fill);
        compute_value!(self, computed, fill_opacity);
        compute_value!(self, computed, fill_rule);
        compute_value!(self, computed, filter);
        compute_value!(self, computed, flood_color);
        compute_value!(self, computed, flood_opacity);
        compute_value!(self, computed, font_family);
        compute_value!(self, computed, font_size);
        compute_value!(self, computed, font_stretch);
        compute_value!(self, computed, font_style);
        compute_value!(self, computed, font_variant);
        compute_value!(self, computed, font_weight);
        compute_value!(self, computed, letter_spacing);
        compute_value!(self, computed, lighting_color);
        compute_value!(self, computed, marker_end);
        compute_value!(self, computed, marker_mid);
        compute_value!(self, computed, marker_start);
        compute_value!(self, computed, mask);
        compute_value!(self, computed, opacity);
        compute_value!(self, computed, overflow);
        compute_value!(self, computed, shape_rendering);
        compute_value!(self, computed, stop_color);
        compute_value!(self, computed, stop_opacity);
        compute_value!(self, computed, stroke);
        compute_value!(self, computed, stroke_dasharray);
        compute_value!(self, computed, stroke_dashoffset);
        compute_value!(self, computed, stroke_line_cap);
        compute_value!(self, computed, stroke_line_join);
        compute_value!(self, computed, stroke_opacity);
        compute_value!(self, computed, stroke_miterlimit);
        compute_value!(self, computed, stroke_width);
        compute_value!(self, computed, text_anchor);
        compute_value!(self, computed, text_decoration);
        compute_value!(self, computed, text_rendering);
        compute_value!(self, computed, unicode_bidi);
        compute_value!(self, computed, visibility);
        compute_value!(self, computed, writing_mode);
        compute_value!(self, computed, xml_lang);
        compute_value!(self, computed, xml_space);
    }

    pub fn is_overflow(&self) -> bool {
        match self.overflow {
            SpecifiedValue::Specified(Overflow::Auto)
            | SpecifiedValue::Specified(Overflow::Visible) => true,
            _ => false,
        }
    }

    fn parse_one_presentation_attribute(
        &mut self,
        attr: QualName,
        value: &str,
    ) -> Result<(), NodeError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        // Presentation attributes don't accept shorthands, e.g. there is no
        // attribute like marker="#foo" and it needs to be set in the style attribute
        // like style="marker: #foo;".  So, pass false for accept_shorthands here.
        match parse_property(&attr, &mut parser, false) {
            Ok(prop) => self.set_parsed_property(&prop),

            // not a presentation attribute; just ignore it
            Err(CssParseError {
                kind: ParseErrorKind::Custom(ValueErrorKind::UnknownProperty),
                ..
            }) => (),

            // https://www.w3.org/TR/CSS2/syndata.html#unsupported-values
            // For all the following cases, ignore illegal values; don't set the whole node to
            // be in error in that case.
            Err(CssParseError {
                kind: ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(ref t)),
                ..
            }) => {
                let mut tok = String::new();

                t.to_css(&mut tok).unwrap(); // FIXME: what do we do with a fmt::Error?
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    \
                     unexpected token '{}')",
                    attr.expanded(),
                    value,
                    tok,
                );
            }

            Err(CssParseError {
                kind: ParseErrorKind::Basic(BasicParseErrorKind::EndOfInput),
                ..
            }) => {
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    \
                     unexpected end of input)",
                    attr.expanded(),
                    value,
                );
            }

            Err(CssParseError {
                kind: ParseErrorKind::Basic(_),
                ..
            }) => {
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    \
                     unexpected error)",
                    attr.expanded(),
                    value,
                );
            }

            Err(CssParseError {
                kind: ParseErrorKind::Custom(ref v),
                ..
            }) => {
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    {})",
                    attr.expanded(),
                    value,
                    v
                );
            }
        }

        Ok(())
    }

    pub fn parse_presentation_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
    ) -> Result<(), NodeError> {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(xml "lang") => {
                    // xml:lang is a non-presentation attribute and as such cannot have the
                    // "inherit" value.  So, we don't call parse_one_presentation_attribute()
                    // for it, but rather call its parser directly.
                    self.xml_lang = SpecifiedValue::Specified(attr.parse(value)?);
                }

                expanded_name!(xml "space") => {
                    // xml:space is a non-presentation attribute and as such cannot have the
                    // "inherit" value.  So, we don't call parse_one_presentation_attribute()
                    // for it, but rather call its parser directly.
                    self.xml_space = SpecifiedValue::Specified(attr.parse(value)?);
                }

                _ => self.parse_one_presentation_attribute(attr, value)?,
            }
        }

        Ok(())
    }

    pub fn set_property_from_declaration(
        &mut self,
        declaration: &Declaration,
        important_styles: &mut HashSet<QualName>,
    ) {
        if !declaration.important && important_styles.contains(&declaration.prop_name) {
            return;
        }

        if declaration.important {
            important_styles.insert(declaration.prop_name.clone());
        }

        self.set_parsed_property(&declaration.property);
    }

    pub fn parse_style_declarations(
        &mut self,
        declarations: &str,
        important_styles: &mut HashSet<QualName>,
    ) -> Result<(), NodeError> {
        let mut input = ParserInput::new(declarations);
        let mut parser = Parser::new(&mut input);

        DeclarationListParser::new(&mut parser, DeclParser)
            .filter_map(Result::ok) // ignore invalid property name or value
            .for_each(|decl| self.set_property_from_declaration(&decl, important_styles));

        Ok(())
    }
}

// Parses the value for the type `T` of the property out of the Parser, including `inherit` values.
fn parse_input<'i, T>(input: &mut Parser<'i, '_>) -> Result<SpecifiedValue<T>, CssParseError<'i>>
where
    T: Property<ComputedValues> + Clone + Default + Parse,
{
    if input
        .try_parse(|p| p.expect_ident_matching("inherit"))
        .is_ok()
    {
        Ok(SpecifiedValue::Inherit)
    } else {
        Parse::parse(input).map(SpecifiedValue::Specified)
    }
}
