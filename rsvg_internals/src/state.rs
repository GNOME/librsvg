use cssparser;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use std::cell::RefCell;
use std::collections::HashSet;
use std::str::FromStr;

use attributes::Attribute;
use color::rgba_to_argb;
use cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use error::*;
use handle::RsvgHandle;
use iri::IRI;
use length::{Dasharray, LengthDir, LengthUnit, RsvgLength};
use node::RsvgNode;
use paint_server::PaintServer;
use parsers::Parse;
use property_bag::PropertyBag;
use property_macros::Property;
use unitinterval::UnitInterval;
use util::{utf8_cstr, utf8_cstr_opt};

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
        match *self {
            SpecifiedValue::Unspecified => {
                if <T as Property<ComputedValues>>::inherits_automatically() {
                    src.clone()
                } else {
                    Default::default()
                }
            }

            SpecifiedValue::Inherit => src.clone(),

            SpecifiedValue::Specified(ref v) => v.compute(src_values),
        }
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

// This is only used as *const RsvgState or *mut RsvgState, as an opaque pointer for C
pub enum RsvgState {}

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
    pub cond: bool,
}

#[derive(Default, Clone)]
pub struct SpecifiedValues {
    pub baseline_shift: SpecifiedValue<BaselineShift>,
    pub clip_path: SpecifiedValue<ClipPath>,
    pub clip_rule: SpecifiedValue<ClipRule>,
    pub comp_op: SpecifiedValue<CompOp>,
    pub color: SpecifiedValue<Color>,
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

// Used to transfer pointers to a ComputedValues to the C code
pub type RsvgComputedValues = *const ComputedValues;

#[derive(Debug, Clone)]
pub struct ComputedValues {
    pub baseline_shift: BaselineShift,
    pub clip_path: ClipPath,
    pub clip_rule: ClipRule,
    pub comp_op: CompOp,
    pub color: Color,
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

impl Default for ComputedValues {
    fn default() -> ComputedValues {
        ComputedValues {
            // please keep these sorted
            baseline_shift: Default::default(),
            clip_path: Default::default(),
            clip_rule: Default::default(),
            color: Default::default(),
            comp_op: Default::default(),
            direction: Default::default(),
            display: Default::default(),
            enable_background: Default::default(),
            fill: Default::default(),
            fill_opacity: Default::default(),
            fill_rule: Default::default(),
            filter: Default::default(),
            flood_color: Default::default(),
            flood_opacity: Default::default(),
            font_family: Default::default(),
            font_size: Default::default(),
            font_stretch: Default::default(),
            font_style: Default::default(),
            font_variant: Default::default(),
            font_weight: Default::default(),
            letter_spacing: Default::default(),
            lighting_color: Default::default(),
            marker_end: Default::default(),
            marker_mid: Default::default(),
            marker_start: Default::default(),
            mask: Default::default(),
            opacity: Default::default(),
            overflow: Default::default(),
            shape_rendering: Default::default(),
            stop_color: Default::default(),
            stop_opacity: Default::default(),
            stroke: Default::default(),
            stroke_dasharray: Default::default(),
            stroke_dashoffset: Default::default(),
            stroke_line_cap: Default::default(),
            stroke_line_join: Default::default(),
            stroke_opacity: Default::default(),
            stroke_miterlimit: Default::default(),
            stroke_width: Default::default(),
            text_anchor: Default::default(),
            text_decoration: Default::default(),
            text_rendering: Default::default(),
            unicode_bidi: Default::default(),
            visibility: Default::default(),
            writing_mode: Default::default(),
            xml_lang: Default::default(),
            xml_space: Default::default(),
        }
    }
}

macro_rules! compute_value {
    ($self:ident, $computed:ident, $name:ident) => {
        $computed.$name = $self.$name.compute(&$computed.$name, &$computed)
    };
}

impl SpecifiedValues {
    fn to_computed_values(&self, computed: &mut ComputedValues) {
        compute_value!(self, computed, baseline_shift);
        compute_value!(self, computed, clip_path);
        compute_value!(self, computed, clip_rule);
        compute_value!(self, computed, comp_op);
        compute_value!(self, computed, color);
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
}

impl State {
    fn new() -> State {
        State {
            values: Default::default(),
            important_styles: Default::default(),
            cond: true,
        }
    }

    fn parse_style_pair(
        &mut self,
        attr: Attribute,
        value: &str,
        important: bool,
        accept_shorthands: bool,
    ) -> Result<(), NodeError> {
        if !important && self.important_styles.borrow().contains(&attr) {
            return Ok(());
        }

        if important {
            self.important_styles.borrow_mut().insert(attr);
        }

        // FIXME: move this to "do catch" when we can bump the rustc version dependency
        let mut parse = || -> Result<(), AttributeError> {
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
                    self.values.font_size = parse_property(value, LengthDir::Both)?;
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
                    self.values.letter_spacing = parse_property(value, LengthDir::Horizontal)?;
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
                    self.values.xml_lang = SpecifiedValue::Specified(XmlLang::parse(value, ())?);
                }

                Attribute::XmlSpace => {
                    // xml:space is not a property; it is a non-presentation attribute and as such
                    // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                    // but rather call its parser directly.
                    self.values.xml_space = SpecifiedValue::Specified(XmlSpace::parse(value, ())?);
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
        // parse().map_err(|e| NodeError::attribute_error(attr, e))

        let _ = parse();

        Ok(())
    }

    fn parse_presentation_attributes(&mut self, pbag: &PropertyBag) -> Result<(), NodeError> {
        for (_key, attr, value) in pbag.iter() {
            self.parse_style_pair(attr, value, false, false)?;
        }

        Ok(())
    }

    pub fn parse_conditional_processing_attributes(
        &mut self,
        pbag: &PropertyBag,
    ) -> Result<(), NodeError> {
        for (_key, attr, value) in pbag.iter() {
            // FIXME: move this to "do catch" when we can bump the rustc version dependency
            let mut parse = || {
                match attr {
                    Attribute::RequiredExtensions if self.cond => {
                        self.cond = RequiredExtensions::parse(value, ())
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    Attribute::RequiredFeatures if self.cond => {
                        self.cond =
                            RequiredFeatures::parse(value, ()).map(|RequiredFeatures(res)| res)?;
                    }

                    Attribute::SystemLanguage if self.cond => {
                        self.cond = SystemLanguage::parse(value, &glib::get_language_names())
                            .map(|SystemLanguage(res, _)| res)?;
                    }

                    _ => {}
                }

                Ok(())
            };

            parse().map_err(|e| NodeError::attribute_error(attr, e))?;
        }

        Ok(())
    }

    fn parse_style_declarations(&mut self, declarations: &str) -> Result<(), NodeError> {
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
                    // Just remove single quotes in a trivial way.  No handling for any
                    // special character inside the quotes is done.  This relates
                    // especially to font-family names.
                    let value = value.replace('\'', "");

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
                        self.parse_style_pair(attr, value, important, true)?;
                    }
                    // else unknown property name; ignore
                }
            }
        }

        Ok(())
    }

    pub fn is_overflow(&self) -> bool {
        match self.values.overflow {
            SpecifiedValue::Specified(Overflow::Auto)
            | SpecifiedValue::Specified(Overflow::Visible) => true,
            _ => false,
        }
    }

    pub fn get_computed_values(&self) -> ComputedValues {
        let mut computed = ComputedValues::default();

        self.to_computed_values(&mut computed);

        computed
    }

    pub fn to_computed_values(&self, values: &mut ComputedValues) {
        self.values.to_computed_values(values);
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
        Parse::parse(value, data).map(SpecifiedValue::Specified)
    }
}

// https://www.w3.org/TR/SVG/text.html#BaselineShiftProperty
make_property!(
    ComputedValues,
    BaselineShift,
    default: RsvgLength::parse("0.0", LengthDir::Both).unwrap(),
    newtype: RsvgLength,
    property_impl: {
        impl Property<ComputedValues> for BaselineShift {
            fn inherits_automatically() -> bool {
                false
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                // FIXME: this implementation has limitations:
                // 1) we only handle 'percent' shifts, but it could also be an absolute offset
                // 2) we should be able to normalize the lengths and add even if they have
                //    different units, but at the moment that requires access to the draw_ctx
                if self.0.unit != LengthUnit::Percent || v.baseline_shift.0.unit != v.font_size.0.unit {
                    return BaselineShift(RsvgLength::new(v.baseline_shift.0.length, v.baseline_shift.0.unit, LengthDir::Both));
                }

                BaselineShift(RsvgLength::new(self.0.length * v.font_size.0.length + v.baseline_shift.0.length, v.font_size.0.unit, LengthDir::Both))
            }
        }
    },
    parse_impl: {
        impl Parse for BaselineShift {
            type Data = ();
            type Err = AttributeError;

            // These values come from Inkscape's SP_CSS_BASELINE_SHIFT_(SUB/SUPER/BASELINE);
            // see sp_style_merge_baseline_shift_from_parent()
            fn parse(s: &str, _: Self::Data) -> Result<BaselineShift, ::error::AttributeError> {
                match s.trim() {
                    "baseline" => Ok(BaselineShift(RsvgLength::new(0.0, LengthUnit::Percent, LengthDir::Both))),
                    "sub" => Ok(BaselineShift(RsvgLength::new(-0.2, LengthUnit::Percent, LengthDir::Both))),
                    "super" => Ok(BaselineShift(RsvgLength::new(0.4, LengthUnit::Percent, LengthDir::Both))),
                    _ => Ok(BaselineShift(RsvgLength::parse(s, LengthDir::Both)?)),
                }
            }
        }
    }
);

make_property!(
    ComputedValues,
    ClipPath,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    ClipRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

// See bgo#764808: we don't inherit CSS from the public API,
// so start off with opaque black instead of transparent.
make_property!(
    ComputedValues,
    Color,
    default: cssparser::RGBA::new(0, 0, 0, 0xff),
    inherits_automatically: true,
    newtype_parse: cssparser::RGBA,
    parse_data_type: ()
);

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

make_property!(
    ComputedValues,
    EnableBackground,
    default: Accumulate,
    inherits_automatically: false,

    identifiers:
    "accumulate" => Accumulate,
    "new" => New,
);

make_property!(
    ComputedValues,
    Fill,
    default: PaintServer::parse("#000", ()).unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    FillOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    ComputedValues,
    FillRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

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
    newtype_from_str: UnitInterval
);

make_property!(
    ComputedValues,
    FontFamily,
    default: "Times New Roman".to_string(),
    inherits_automatically: true,
    newtype_from_str: String
);

make_property!(
    ComputedValues,
    FontSize,
    default: RsvgLength::parse("12.0", LengthDir::Both).unwrap(),
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir,
    property_impl: {
        impl Property<ComputedValues> for FontSize {
            fn inherits_automatically() -> bool {
                true
            }

            fn compute(&self, v: &ComputedValues) -> Self {
                match self.0.unit {
                    LengthUnit::Percent =>
                        FontSize(RsvgLength::new(self.0.length * v.font_size.0.length, v.font_size.0.unit, LengthDir::Both)),

                    LengthUnit::RelativeLarger =>
                        FontSize(RsvgLength::new(v.font_size.0.length * 1.2, v.font_size.0.unit, LengthDir::Both)),

                    LengthUnit::RelativeSmaller =>
                        FontSize(RsvgLength::new(v.font_size.0.length / 1.2, v.font_size.0.unit, LengthDir::Both)),

                    _ => self.clone(),
                }
            }
        }
    }
);

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

make_property!(
    ComputedValues,
    FontVariant,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "small-caps" => SmallCaps,
);

make_property!(
    ComputedValues,
    FontWeight,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "bold" => Bold,
    "bolder" => Bolder,
    "lighter" => Lighter,
    "100" => W100, // FIXME: we should use Weight(100),
    "200" => W200, // but we need a smarter macro for that
    "300" => W300,
    "400" => W400,
    "500" => W500,
    "600" => W600,
    "700" => W700,
    "800" => W800,
    "900" => W900,
);

make_property!(
    ComputedValues,
    LetterSpacing,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
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

make_property!(
    ComputedValues,
    MarkerEnd,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    MarkerMid,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    MarkerStart,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    Mask,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    Opacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_from_str: UnitInterval
);

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
    newtype_from_str: UnitInterval
);

make_property!(
    ComputedValues,
    Stroke,
    default: PaintServer::None,
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    StrokeDasharray,
    default: Dasharray::default(),
    inherits_automatically: true,
    newtype_parse: Dasharray,
    parse_data_type: ()
);

make_property!(
    ComputedValues,
    StrokeDashoffset,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

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

make_property!(
    ComputedValues,
    StrokeOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    ComputedValues,
    StrokeMiterlimit,
    default: 4f64,
    inherits_automatically: true,
    newtype_from_str: f64
);

make_property!(
    ComputedValues,
    StrokeWidth,
    default: RsvgLength::parse("1.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

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
            type Err = AttributeError;

            fn parse(s: &str, _: Self::Data) -> Result<TextDecoration, AttributeError> {
                Ok(TextDecoration {
                    overline: s.contains("overline"),
                    underline: s.contains("underline"),
                    strike: s.contains("strike") || s.contains("line-through"),
                })
            }
        }
    }
);

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
    newtype_from_str: String
);

make_property!(
    ComputedValues,
    XmlSpace,
    default: Default,
    inherits_automatically: true,

    identifiers:
    "default" => Default,
    "preserve" => Preserve,
);

pub fn from_c<'a>(state: *const RsvgState) -> &'a State {
    assert!(!state.is_null());

    unsafe { &*(state as *const State) }
}

pub fn from_c_mut<'a>(state: *mut RsvgState) -> &'a mut State {
    assert!(!state.is_null());

    unsafe { &mut *(state as *mut State) }
}

pub fn to_c_mut(state: &mut State) -> *mut RsvgState {
    state as *mut State as *mut RsvgState
}

// Rust State API for consumption from C ----------------------------------------

#[no_mangle]
pub extern "C" fn rsvg_state_new() -> *mut RsvgState {
    Box::into_raw(Box::new(State::new())) as *mut RsvgState
}

#[no_mangle]
pub extern "C" fn rsvg_state_free(state: *mut RsvgState) {
    let state = from_c_mut(state);

    unsafe {
        Box::from_raw(state);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_parse_style_pair(
    state: *mut RsvgState,
    attr: Attribute,
    value: *const libc::c_char,
    important: glib_sys::gboolean,
    accept_shorthands: glib_sys::gboolean,
) -> glib_sys::gboolean {
    let state = from_c_mut(state);

    assert!(!value.is_null());

    let value = unsafe { utf8_cstr(value) };

    match state.parse_style_pair(
        attr,
        value,
        from_glib(important),
        from_glib(accept_shorthands),
    ) {
        Ok(_) => true.to_glib(),
        Err(_) => false.to_glib(),
    }
}

extern "C" {
    fn rsvg_lookup_apply_css_style(
        handle: *const RsvgHandle,
        target: *const libc::c_char,
        state: *mut RsvgState,
    ) -> glib_sys::gboolean;
}

#[no_mangle]
pub extern "C" fn rsvg_parse_style_attrs(
    handle: *const RsvgHandle,
    raw_node: *const RsvgNode,
    tag: *const libc::c_char,
    klazz: *const libc::c_char,
    id: *const libc::c_char,
    pbag: *const PropertyBag,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    let tag = unsafe { utf8_cstr(tag) };

    let klazz = unsafe { utf8_cstr_opt(klazz) };
    let id = unsafe { utf8_cstr_opt(id) };

    let pbag = unsafe { &*pbag };

    parse_style_attrs(handle, node, tag, klazz, id, pbag);
}

// Sets the node's state from the attributes in the pbag.  Also
// applies CSS rules in our limited way based on the node's
// tag/klazz/id.
fn parse_style_attrs(
    handle: *const RsvgHandle,
    node: &RsvgNode,
    tag: &str,
    klazz: Option<&str>,
    id: Option<&str>,
    pbag: &PropertyBag,
) {
    let state = node.get_state_mut();

    match state.parse_presentation_attributes(pbag) {
        Ok(_) => (),
        Err(_) => (),
        /* FIXME: we'll ignore errors here for now.  If we return, we expose
         * buggy handling of the enable-background property; we are not parsing it correctly.
         * This causes tests/fixtures/reftests/bugs/587721-text-transform.svg to fail
         * because it has enable-background="new 0 0 1179.75118 687.74173" in the toplevel svg
         * element.
         *        Err(e) => (),
         *        {
         *            node.set_error(e);
         *            return;
         *        } */
    }

    match state.parse_conditional_processing_attributes(pbag) {
        Ok(_) => (),
        Err(e) => {
            node.set_error(e);
            return;
        }
    }

    // Try to properly support all of the following, including inheritance:
    // *
    // #id
    // tag
    // tag#id
    // tag.class
    // tag.class#id
    //
    // This is basically a semi-compliant CSS2 selection engine

    unsafe {
        // *
        rsvg_lookup_apply_css_style(handle, "*".to_glib_none().0, to_c_mut(state));

        // tag
        rsvg_lookup_apply_css_style(handle, tag.to_glib_none().0, to_c_mut(state));

        if let Some(klazz) = klazz {
            for cls in klazz.split_whitespace() {
                let mut found = false;

                if !cls.is_empty() {
                    // tag.class#id
                    if let Some(id) = id {
                        let target = format!("{}.{}#{}", tag, cls, id);
                        found = found
                            || from_glib(rsvg_lookup_apply_css_style(
                                handle,
                                target.to_glib_none().0,
                                to_c_mut(state),
                            ));
                    }

                    // .class#id
                    if let Some(id) = id {
                        let target = format!(".{}#{}", cls, id);
                        found = found
                            || from_glib(rsvg_lookup_apply_css_style(
                                handle,
                                target.to_glib_none().0,
                                to_c_mut(state),
                            ));
                    }

                    // tag.class
                    let target = format!("{}.{}", tag, cls);
                    found = found
                        || from_glib(rsvg_lookup_apply_css_style(
                            handle,
                            target.to_glib_none().0,
                            to_c_mut(state),
                        ));

                    if !found {
                        // didn't find anything more specific, just apply the class style
                        let target = format!(".{}", cls);
                        rsvg_lookup_apply_css_style(
                            handle,
                            target.to_glib_none().0,
                            to_c_mut(state),
                        );
                    }
                }
            }
        }

        if let Some(id) = id {
            // id
            let target = format!("#{}", id);
            rsvg_lookup_apply_css_style(handle, target.to_glib_none().0, to_c_mut(state));

            // tag#id
            let target = format!("{}#{}", tag, id);
            rsvg_lookup_apply_css_style(handle, target.to_glib_none().0, to_c_mut(state));
        }

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Style => {
                    if let Err(e) = state.parse_style_declarations(value) {
                        node.set_error(e);
                        break;
                    }
                }

                _ => (),
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_computed_values_get_flood_color_argb(values: RsvgComputedValues) -> u32 {
    assert!(!values.is_null());
    let values = unsafe { &*values };

    match values.flood_color {
        FloodColor(cssparser::Color::CurrentColor) => rgba_to_argb(values.color.0),
        FloodColor(cssparser::Color::RGBA(ref rgba)) => rgba_to_argb(*rgba),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_computed_values_get_flood_opacity(values: RsvgComputedValues) -> u8 {
    assert!(!values.is_null());
    let values = unsafe { &*values };

    u8::from(values.flood_opacity.0)
}

#[no_mangle]
pub extern "C" fn rsvg_computed_values_get_lighting_color_argb(values: RsvgComputedValues) -> u32 {
    assert!(!values.is_null());
    let values = unsafe { &*values };

    match values.lighting_color {
        LightingColor(cssparser::Color::CurrentColor) => rgba_to_argb(values.color.0),
        LightingColor(cssparser::Color::RGBA(ref rgba)) => rgba_to_argb(*rgba),
    }
}
