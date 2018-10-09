use cssparser::{self, Parser, Token};
use std::cell::RefCell;
use std::collections::HashSet;
use std::str::FromStr;

use attributes::Attribute;
use error::*;
use font_props::{FontSizeSpec, FontWeightSpec, LetterSpacingSpec, SingleFontFamily};
use iri::IRI;
use length::{Dasharray, Length, LengthDir, LengthUnit};
use paint_server::PaintServer;
use parsers::{Parse, ParseError};
use property_bag::PropertyBag;
use property_macros::Property;
use unitinterval::UnitInterval;

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

/// Holds the state of CSS properties
///
/// This is used for various purposes:
///
/// * Immutably, to store the attributes of element nodes after parsing.
/// * Mutably, during cascading/rendering.
///
/// Each property should have its own data type, and implement
/// `Default` and `parsers::Parse`.
///
/// If a property is `None`, is means it was not specified and must be
/// inherited from the parent state, or in the end the caller can
/// `.unwrap_or_default()` to get the default value for the property.

pub struct State {
    pub values: SpecifiedValues,
    important_styles: RefCell<HashSet<Attribute>>,
}

#[derive(Default, Clone)]
pub struct SpecifiedValues {
    pub baseline_shift: SpecifiedValue<BaselineShift>,
    pub clip_path: SpecifiedValue<ClipPath>,
    pub clip_rule: SpecifiedValue<ClipRule>,
    pub comp_op: SpecifiedValue<CompOp>,
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
    pub comp_op: CompOp,
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

    pub fn text_gravity_is_vertical(&self) -> bool {
        match self.writing_mode {
            WritingMode::Tb | WritingMode::TbRl => true,
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
    pub fn to_computed_values(&self, computed: &mut ComputedValues) {
        compute_value!(self, computed, baseline_shift);
        compute_value!(self, computed, clip_path);
        compute_value!(self, computed, clip_rule);
        compute_value!(self, computed, comp_op);
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
}

impl State {
    pub fn new() -> State {
        State {
            values: Default::default(),
            important_styles: Default::default(),
        }
    }

    fn parse_attribute_pair(
        &mut self,
        attr: Attribute,
        value: &str,
        accept_shorthands: bool,
    ) -> Result<(), NodeError> {
        // FIXME: move this to "do catch" when we can bump the rustc version dependency
        let mut parse = || -> Result<(), ValueErrorKind> {
            // please keep these sorted
            match attr {
                Attribute::BaselineShift => {
                    self.values.baseline_shift = parse_property(value, ())?;
                }

                Attribute::ClipPath => {
                    self.values.clip_path = parse_property(value, ())?;
                }

                Attribute::ClipRule => {
                    self.values.clip_rule = parse_property(value, ())?;
                }

                Attribute::Color => {
                    self.values.color = parse_property(value, ())?;
                }

                Attribute::ColorInterpolationFilters => {
                    self.values.color_interpolation_filters = parse_property(value, ())?;
                }

                Attribute::CompOp => {
                    self.values.comp_op = parse_property(value, ())?;
                }

                Attribute::Direction => {
                    self.values.direction = parse_property(value, ())?;
                }

                Attribute::Display => {
                    self.values.display = parse_property(value, ())?;
                }

                Attribute::EnableBackground => {
                    self.values.enable_background = parse_property(value, ())?;
                }

                Attribute::Fill => {
                    self.values.fill = parse_property(value, ())?;
                }

                Attribute::FillOpacity => {
                    self.values.fill_opacity = parse_property(value, ())?;
                }

                Attribute::FillRule => {
                    self.values.fill_rule = parse_property(value, ())?;
                }

                Attribute::Filter => {
                    self.values.filter = parse_property(value, ())?;
                }

                Attribute::FloodColor => {
                    self.values.flood_color = parse_property(value, ())?;
                }

                Attribute::FloodOpacity => {
                    self.values.flood_opacity = parse_property(value, ())?;
                }

                Attribute::FontFamily => {
                    self.values.font_family = parse_property(value, ())?;
                }

                Attribute::FontSize => {
                    self.values.font_size = parse_property(value, ())?;
                }

                Attribute::FontStretch => {
                    self.values.font_stretch = parse_property(value, ())?;
                }

                Attribute::FontStyle => {
                    self.values.font_style = parse_property(value, ())?;
                }

                Attribute::FontVariant => {
                    self.values.font_variant = parse_property(value, ())?;
                }

                Attribute::FontWeight => {
                    self.values.font_weight = parse_property(value, ())?;
                }

                Attribute::LetterSpacing => {
                    self.values.letter_spacing = parse_property(value, ())?;
                }

                Attribute::LightingColor => {
                    self.values.lighting_color = parse_property(value, ())?;
                }

                Attribute::MarkerEnd => {
                    self.values.marker_end = parse_property(value, ())?;
                }

                Attribute::MarkerMid => {
                    self.values.marker_mid = parse_property(value, ())?;
                }

                Attribute::MarkerStart => {
                    self.values.marker_start = parse_property(value, ())?;
                }

                Attribute::Marker if accept_shorthands => {
                    self.values.marker_end = parse_property(value, ())?;
                    self.values.marker_mid = parse_property(value, ())?;
                    self.values.marker_start = parse_property(value, ())?;
                }

                Attribute::Mask => {
                    self.values.mask = parse_property(value, ())?;
                }

                Attribute::Opacity => {
                    self.values.opacity = parse_property(value, ())?;
                }

                Attribute::Overflow => {
                    self.values.overflow = parse_property(value, ())?;
                }

                Attribute::ShapeRendering => {
                    self.values.shape_rendering = parse_property(value, ())?;
                }

                Attribute::StopColor => {
                    self.values.stop_color = parse_property(value, ())?;
                }

                Attribute::StopOpacity => {
                    self.values.stop_opacity = parse_property(value, ())?;
                }

                Attribute::Stroke => {
                    self.values.stroke = parse_property(value, ())?;
                }

                Attribute::StrokeDasharray => {
                    self.values.stroke_dasharray = parse_property(value, ())?;
                }

                Attribute::StrokeDashoffset => {
                    self.values.stroke_dashoffset = parse_property(value, LengthDir::Both)?;
                }

                Attribute::StrokeLinecap => {
                    self.values.stroke_line_cap = parse_property(value, ())?;
                }

                Attribute::StrokeLinejoin => {
                    self.values.stroke_line_join = parse_property(value, ())?;
                }

                Attribute::StrokeOpacity => {
                    self.values.stroke_opacity = parse_property(value, ())?;
                }

                Attribute::StrokeMiterlimit => {
                    self.values.stroke_miterlimit = parse_property(value, ())?;
                }

                Attribute::StrokeWidth => {
                    self.values.stroke_width = parse_property(value, LengthDir::Both)?;
                }

                Attribute::TextAnchor => {
                    self.values.text_anchor = parse_property(value, ())?;
                }

                Attribute::TextDecoration => {
                    self.values.text_decoration = parse_property(value, ())?;
                }

                Attribute::TextRendering => {
                    self.values.text_rendering = parse_property(value, ())?;
                }

                Attribute::UnicodeBidi => {
                    self.values.unicode_bidi = parse_property(value, ())?;
                }

                Attribute::Visibility => {
                    self.values.visibility = parse_property(value, ())?;
                }

                Attribute::WritingMode => {
                    self.values.writing_mode = parse_property(value, ())?;
                }

                Attribute::XmlLang => {
                    // xml:lang is not a property; it is a non-presentation attribute and as such
                    // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                    // but rather call its parser directly.
                    self.values.xml_lang =
                        SpecifiedValue::Specified(XmlLang::parse_str(value, ())?);
                }

                Attribute::XmlSpace => {
                    // xml:space is not a property; it is a non-presentation attribute and as such
                    // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                    // but rather call its parser directly.
                    self.values.xml_space =
                        SpecifiedValue::Specified(XmlSpace::parse_str(value, ())?);
                }

                _ => {
                    // Maybe it's an attribute not parsed here, but in the
                    // node implementations.
                }
            }

            Ok(())
        };

        // https://www.w3.org/TR/CSS2/syndata.html#unsupported-values
        // Ignore unsupported / illegal values; don't set the whole
        // node to be in error in that case.

        if let Err(e) = parse().map_err(|e| NodeError::attribute_error(attr, e)) {
            rsvg_log!(
                "(style property error for attribute {:?}\n    value=\"{}\"\n    {}\n    property \
                 will be ignored)",
                attr,
                value,
                e
            );
        }

        // If we didn't ignore property errors, we could just return this:
        // parse().map_err(|e| NodeError::attribute_error(attr, e))
        Ok(())
    }

    pub fn parse_presentation_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
    ) -> Result<(), NodeError> {
        for (_key, attr, value) in pbag.iter() {
            self.parse_attribute_pair(attr, value, false)?;
        }

        Ok(())
    }

    pub fn parse_style_pair(
        &mut self,
        attr: Attribute,
        value: &str,
        important: bool,
    ) -> Result<(), NodeError> {
        if !important && self.important_styles.borrow().contains(&attr) {
            return Ok(());
        }

        if important {
            self.important_styles.borrow_mut().insert(attr);
        }

        self.parse_attribute_pair(attr, value, true)
    }

    pub fn parse_style_declarations(&mut self, declarations: &str) -> Result<(), NodeError> {
        // Split an attribute value like style="foo: bar; baz: beep;" into
        // individual CSS declarations ("foo: bar" and "baz: beep") and
        // set them onto the state struct.
        //
        // FIXME: It's known that this is _way_ out of spec. A more complete
        // CSS2 implementation will happen later.

        for decl in declarations.split(';') {
            if let Some(colon_pos) = decl.find(':') {
                let (prop_name, value) = decl.split_at(colon_pos);

                let prop_name = prop_name.trim();
                let value = value[1..].trim();

                if !prop_name.is_empty() && !value.is_empty() {
                    let mut important = false;

                    let value = if let Some(bang_pos) = value.find('!') {
                        let (before_bang, bang_and_after) = value.split_at(bang_pos);

                        if bang_and_after[1..].trim() == "important" {
                            important = true;
                        }

                        before_bang.trim()
                    } else {
                        &value
                    };

                    if let Ok(attr) = Attribute::from_str(prop_name) {
                        self.parse_style_pair(attr, value, important)?;
                    }
                    // else unknown property name; ignore
                }
            }
        }

        Ok(())
    }

    pub fn get_specified_values(&self) -> &SpecifiedValues {
        &self.values
    }
}

// Parses the `value` for the type `T` of the property, including `inherit` values.
//
// If the `value` is `inherit`, returns `Ok(None)`; otherwise returns
// `Ok(Some(T))`.
fn parse_property<T>(
    value: &str,
    data: <T as Parse>::Data,
) -> Result<SpecifiedValue<T>, <T as Parse>::Err>
where
    T: Property<ComputedValues> + Clone + Default + Parse,
{
    if value.trim() == "inherit" {
        Ok(SpecifiedValue::Inherit)
    } else {
        Parse::parse_str(value, data).map(SpecifiedValue::Specified)
    }
}

// https://www.w3.org/TR/SVG/text.html#BaselineShiftProperty
make_property!(
    ComputedValues,
    BaselineShift,
    default: Length::parse_str("0.0", LengthDir::Both).unwrap(),
    newtype: Length,
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
                    return BaselineShift(Length::new(v.baseline_shift.0.length, v.baseline_shift.0.unit, LengthDir::Both));
                }

                BaselineShift(Length::new(self.0.length * font_size.length + v.baseline_shift.0.length, font_size.unit, LengthDir::Both))
            }
        }
    },
    parse_impl: {
        impl Parse for BaselineShift {
            type Data = ();
            type Err = ValueErrorKind;

            // These values come from Inkscape's SP_CSS_BASELINE_SHIFT_(SUB/SUPER/BASELINE);
            // see sp_style_merge_baseline_shift_from_parent()
            fn parse(parser: &mut Parser<'_, '_>, _: Self::Data) -> Result<BaselineShift, ::error::ValueErrorKind> {
                let parser_state = parser.state();

                {
                    let token = parser.next().map_err(|_| ::error::ValueErrorKind::Parse(
                        ::parsers::ParseError::new("expected token"),
                    ))?;

                    if let Token::Ident(ref cow) = token {
                        match cow.as_ref() {
                            "baseline" => return Ok(BaselineShift(
                                Length::new(0.0, LengthUnit::Percent, LengthDir::Both)
                            )),

                            "sub" => return Ok(BaselineShift(
                                Length::new(-0.2, LengthUnit::Percent, LengthDir::Both)
                            )),

                            "super" => return Ok(BaselineShift(
                                Length::new(0.4, LengthUnit::Percent, LengthDir::Both),
                            )),

                            _ => (),
                        }
                    }
                }

                parser.reset(&parser_state);

                Ok(BaselineShift(Length::from_cssparser(parser, LengthDir::Both)?))
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
    parse_data_type: ()
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
    parse_data_type: ()
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

// https://gitlab.gnome.org/GNOME/librsvg/issues/268 - can we remove this property?
make_property!(
    ComputedValues,
    CompOp,
    default: SrcOver,
    inherits_automatically: false,

    identifiers:
    "clear" => Clear,
    "src" => Src,
    "dst" => Dst,
    "src-over" => SrcOver,
    "dst-over" => DstOver,
    "src-in" => SrcIn,
    "dst-in" => DstIn,
    "src-out" => SrcOut,
    "dst-out" => DstOut,
    "src-atop" => SrcAtop,
    "dst-atop" => DstAtop,
    "xor" => Xor,
    "plus" => Plus,
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
    default: PaintServer::parse_str("#000", ()).unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#FillOpacityProperty
make_property!(
    ComputedValues,
    FillOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_parse: UnitInterval,
    parse_data_type: ()
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
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/filters.html#FloodColorProperty
make_property!(
    ComputedValues,
    FloodColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/filters.html#FloodOpacityProperty
make_property!(
    ComputedValues,
    FloodOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/text.html#FontFamilyProperty
make_property!(
    ComputedValues,
    FontFamily,
    default: SingleFontFamily("Times New Roman".to_string()),
    inherits_automatically: true,
    newtype_parse: SingleFontFamily,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/text.html#FontSizeProperty
make_property!(
    ComputedValues,
    FontSize,
    default: FontSizeSpec::Value(Length::parse_str("12.0", LengthDir::Both).unwrap()),
    newtype_parse: FontSizeSpec,
    parse_data_type: (),
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
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/text.html#LetterSpacingProperty
make_property!(
    ComputedValues,
    LetterSpacing,
    default: LetterSpacingSpec::Normal,
    newtype_parse: LetterSpacingSpec,
    parse_data_type: (),
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
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#MarkerEndProperty
make_property!(
    ComputedValues,
    MarkerEnd,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#MarkerMidProperty
make_property!(
    ComputedValues,
    MarkerMid,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#MarkerStartProperty
make_property!(
    ComputedValues,
    MarkerStart,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/masking.html#MaskProperty
make_property!(
    ComputedValues,
    Mask,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/masking.html#OpacityProperty
make_property!(
    ComputedValues,
    Opacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
    parse_data_type: ()
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
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/pservers.html#StopOpacityProperty
make_property!(
    ComputedValues,
    StopOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_parse: UnitInterval,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#StrokeProperty
make_property!(
    ComputedValues,
    Stroke,
    default: PaintServer::None,
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#StrokeDasharrayProperty
make_property!(
    ComputedValues,
    StrokeDasharray,
    default: Dasharray::default(),
    inherits_automatically: true,
    newtype_parse: Dasharray,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#StrokeDashoffsetProperty
make_property!(
    ComputedValues,
    StrokeDashoffset,
    default: Length::default(),
    inherits_automatically: true,
    newtype_parse: Length,
    parse_data_type: LengthDir
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
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#StrokeOpacityProperty
make_property!(
    ComputedValues,
    StrokeOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_parse: UnitInterval,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/painting.html#StrokeWidthProperty
make_property!(
    ComputedValues,
    StrokeWidth,
    default: Length::parse_str("1.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype_parse: Length,
    parse_data_type: LengthDir
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
            type Data = ();
            type Err = ValueErrorKind;

            fn parse(parser: &mut Parser<'_, '_>, _: Self::Data) -> Result<TextDecoration, ValueErrorKind> {
                let mut overline = false;
                let mut underline = false;
                let mut strike = false;

                if parser.try(|p| p.expect_ident_matching("none")).is_ok() {
                    return Ok(TextDecoration::default());
                }

                while !parser.is_exhausted() {
                    let cow = parser.expect_ident().map_err(|_| ::error::ValueErrorKind::Parse(
                        ::parsers::ParseError::new("expected identifier"),
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
        TextDecoration::parse_str("none", ()).unwrap(),
        TextDecoration {
            overline: false,
            underline: false,
            strike: false,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("overline", ()).unwrap(),
        TextDecoration {
            overline: true,
            underline: false,
            strike: false,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("underline", ()).unwrap(),
        TextDecoration {
            overline: false,
            underline: true,
            strike: false,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("line-through", ()).unwrap(),
        TextDecoration {
            overline: false,
            underline: false,
            strike: true,
        }
    );

    assert_eq!(
        TextDecoration::parse_str("underline overline", ()).unwrap(),
        TextDecoration {
            overline: true,
            underline: true,
            strike: false,
        }
    );

    assert!(TextDecoration::parse_str("airline", ()).is_err())
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

make_property!(
    ComputedValues,
    XmlLang,
    default: "".to_string(), // see create_pango_layout()
    inherits_automatically: true,
    newtype: String,
    parse_impl: {
        impl Parse for XmlLang {
            type Data = ();
            type Err = ValueErrorKind;

            fn parse(
                parser: &mut Parser<'_, '_>,
                _: Self::Data
            ) -> Result<XmlLang, ValueErrorKind> {
                Ok(XmlLang(parser.expect_ident()?.to_string()))
            }
        }
    },
    parse_data_type: ()
);

#[cfg(test)]
#[test]
fn parses_xml_lang() {
    assert_eq!(
        XmlLang::parse_str("es-MX", ()).unwrap(),
        XmlLang("es-MX".to_string())
    );

    assert!(XmlLang::parse_str("", ()).is_err());
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
