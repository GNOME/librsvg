use cairo::{self, MatrixTrait};
use cssparser;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr;
use std::str::FromStr;

use attributes::Attribute;
use cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use error::*;
use handle::RsvgHandle;
use iri::IRI;
use length::{Dasharray, LengthDir, RsvgLength};
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
    T: Property + Clone + Default,
{
    Unspecified,
    Inherit,
    Specified(T),
}

impl<T> SpecifiedValue<T>
where
    T: Property + Clone + Default,
{
    pub fn inherit_from(&self, src: &T) -> T {
        match *self {
            SpecifiedValue::Unspecified => {
                if <T as Property>::inherits_automatically() {
                    src.clone()
                } else {
                    Default::default()
                }
            }

            SpecifiedValue::Inherit => src.clone(),

            SpecifiedValue::Specified(ref v) => v.clone(),
        }
    }
}

impl<T> Default for SpecifiedValue<T>
where
    T: Default + Property + Clone,
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

// FIXME: #[derive(Clone)] is not correct here; states are not meant
// to be cloned.  We should remove this when we remove the hack in
// state_reinherit_top(), to clone_from() while preserving the parent
#[derive(Clone)]
pub struct State {
    pub parent: *const RsvgState,

    pub affine: cairo::Matrix,

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

#[derive(Clone)]
pub struct ComputedValues {
    pub affine: cairo::Matrix,
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
            affine: cairo::Matrix::identity(),

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

macro_rules! inherit_from {
    ($self:ident, $computed:ident, $name:ident) => {
        $computed.$name = $self.$name.inherit_from(&$computed.$name)
    };
}

impl SpecifiedValues {
    fn to_computed_values(&self, computed: &mut ComputedValues) {
        inherit_from!(self, computed, baseline_shift);
        inherit_from!(self, computed, clip_path);
        inherit_from!(self, computed, clip_rule);
        inherit_from!(self, computed, comp_op);
        inherit_from!(self, computed, color);
        inherit_from!(self, computed, direction);
        inherit_from!(self, computed, display);
        inherit_from!(self, computed, enable_background);
        inherit_from!(self, computed, fill);
        inherit_from!(self, computed, fill_opacity);
        inherit_from!(self, computed, fill_rule);
        inherit_from!(self, computed, filter);
        inherit_from!(self, computed, flood_color);
        inherit_from!(self, computed, flood_opacity);
        inherit_from!(self, computed, font_family);
        inherit_from!(self, computed, font_size);
        inherit_from!(self, computed, font_stretch);
        inherit_from!(self, computed, font_style);
        inherit_from!(self, computed, font_variant);
        inherit_from!(self, computed, font_weight);
        inherit_from!(self, computed, letter_spacing);
        inherit_from!(self, computed, lighting_color);
        inherit_from!(self, computed, marker_end);
        inherit_from!(self, computed, marker_mid);
        inherit_from!(self, computed, marker_start);
        inherit_from!(self, computed, mask);
        inherit_from!(self, computed, opacity);
        inherit_from!(self, computed, overflow);
        inherit_from!(self, computed, shape_rendering);
        inherit_from!(self, computed, stop_color);
        inherit_from!(self, computed, stop_opacity);
        inherit_from!(self, computed, stroke);
        inherit_from!(self, computed, stroke_dasharray);
        inherit_from!(self, computed, stroke_dashoffset);
        inherit_from!(self, computed, stroke_line_cap);
        inherit_from!(self, computed, stroke_line_join);
        inherit_from!(self, computed, stroke_opacity);
        inherit_from!(self, computed, stroke_miterlimit);
        inherit_from!(self, computed, stroke_width);
        inherit_from!(self, computed, text_anchor);
        inherit_from!(self, computed, text_decoration);
        inherit_from!(self, computed, text_rendering);
        inherit_from!(self, computed, unicode_bidi);
        inherit_from!(self, computed, visibility);
        inherit_from!(self, computed, writing_mode);
        inherit_from!(self, computed, xml_lang);
        inherit_from!(self, computed, xml_space);
    }
}

impl State {
    pub fn new_with_parent(parent: Option<&State>) -> State {
        if let Some(parent) = parent {
            State::new(to_c(parent))
        } else {
            State::new(ptr::null())
        }
    }

    fn new(parent: *const RsvgState) -> State {
        State {
            parent,

            affine: cairo::Matrix::identity(),

            values: Default::default(),

            important_styles: Default::default(),
            cond: true,
        }
    }

    pub fn parent<'a>(&self) -> Option<&'a State> {
        if self.parent.is_null() {
            None
        } else {
            Some(from_c(self.parent))
        }
    }

    pub fn reinherit(&mut self, src: &State) {
        self.inherit_run(src, State::reinheritfunction, false);
    }

    pub fn inherit(&mut self, src: &State) {
        self.inherit_run(src, State::inheritfunction, true);
    }

    pub fn force(&mut self, src: &State) {
        self.inherit_run(src, State::forcefunction, false);
    }

    pub fn dominate(&mut self, src: &State) {
        self.inherit_run(src, State::dominatefunction, false);
    }

    pub fn reconstruct(&mut self, node: &RsvgNode) {
        if let Some(parent) = node.get_parent() {
            self.reconstruct(&parent);
            self.inherit(node.get_state());
        }
    }

    // reinherit is given dst which is the top of the state stack
    // and src which is the layer before in the state stack from
    // which it should be inherited
    fn reinheritfunction(dst: bool, _src: bool) -> bool {
        if !dst {
            true
        } else {
            false
        }
    }

    // put something new on the inheritance stack, dst is the top of the stack,
    // src is the state to be integrated, this is essentially the opposite of
    // reinherit, because it is being given stuff to be integrated on the top,
    // rather than the context underneath.
    fn inheritfunction(_dst: bool, src: bool) -> bool {
        src
    }

    // copy everything inheritable from the src to the dst */
    fn forcefunction(_dst: bool, _src: bool) -> bool {
        true
    }

    // dominate is given dst which is the top of the state stack and
    // src which is the layer before in the state stack from which it
    // should be inherited from, however if anything is directly
    // specified in src (the second last layer) it will override
    // anything on the top layer, this is for overrides in <use> tags
    fn dominatefunction(dst: bool, src: bool) -> bool {
        if !dst || src {
            true
        } else {
            false
        }
    }

    fn inherit_run(
        &mut self,
        src: &State,
        inherit_fn: fn(bool, bool) -> bool,
        inherituninheritables: bool,
    ) {
        // please keep these sorted
        inherit(
            inherit_fn,
            &mut self.values.baseline_shift,
            &src.values.baseline_shift,
        );
        inherit(
            inherit_fn,
            &mut self.values.clip_rule,
            &src.values.clip_rule,
        );
        inherit(inherit_fn, &mut self.values.color, &src.values.color);
        inherit(
            inherit_fn,
            &mut self.values.direction,
            &src.values.direction,
        );
        inherit(inherit_fn, &mut self.values.display, &src.values.display);
        inherit(inherit_fn, &mut self.values.fill, &src.values.fill);
        inherit(
            inherit_fn,
            &mut self.values.fill_opacity,
            &src.values.fill_opacity,
        );
        inherit(
            inherit_fn,
            &mut self.values.fill_rule,
            &src.values.fill_rule,
        );
        inherit(
            inherit_fn,
            &mut self.values.flood_color,
            &src.values.flood_color,
        );
        inherit(
            inherit_fn,
            &mut self.values.flood_opacity,
            &src.values.flood_opacity,
        );
        inherit(
            inherit_fn,
            &mut self.values.font_family,
            &src.values.font_family,
        );
        inherit(
            inherit_fn,
            &mut self.values.font_size,
            &src.values.font_size,
        );
        inherit(
            inherit_fn,
            &mut self.values.font_stretch,
            &src.values.font_stretch,
        );
        inherit(
            inherit_fn,
            &mut self.values.font_style,
            &src.values.font_style,
        );
        inherit(
            inherit_fn,
            &mut self.values.font_variant,
            &src.values.font_variant,
        );
        inherit(
            inherit_fn,
            &mut self.values.font_weight,
            &src.values.font_weight,
        );
        inherit(
            inherit_fn,
            &mut self.values.letter_spacing,
            &src.values.letter_spacing,
        );
        inherit(
            inherit_fn,
            &mut self.values.lighting_color,
            &src.values.lighting_color,
        );
        inherit(
            inherit_fn,
            &mut self.values.marker_end,
            &src.values.marker_end,
        );
        inherit(
            inherit_fn,
            &mut self.values.marker_mid,
            &src.values.marker_mid,
        );
        inherit(
            inherit_fn,
            &mut self.values.marker_start,
            &src.values.marker_start,
        );
        inherit(inherit_fn, &mut self.values.overflow, &src.values.overflow);
        inherit(
            inherit_fn,
            &mut self.values.shape_rendering,
            &src.values.shape_rendering,
        );
        inherit(
            inherit_fn,
            &mut self.values.stop_color,
            &src.values.stop_color,
        );
        inherit(
            inherit_fn,
            &mut self.values.stop_opacity,
            &src.values.stop_opacity,
        );
        inherit(inherit_fn, &mut self.values.stroke, &src.values.stroke);
        inherit(
            inherit_fn,
            &mut self.values.stroke_dasharray,
            &src.values.stroke_dasharray,
        );
        inherit(
            inherit_fn,
            &mut self.values.stroke_dashoffset,
            &src.values.stroke_dashoffset,
        );
        inherit(
            inherit_fn,
            &mut self.values.stroke_line_cap,
            &src.values.stroke_line_cap,
        );
        inherit(
            inherit_fn,
            &mut self.values.stroke_line_join,
            &src.values.stroke_line_join,
        );
        inherit(
            inherit_fn,
            &mut self.values.stroke_opacity,
            &src.values.stroke_opacity,
        );
        inherit(
            inherit_fn,
            &mut self.values.stroke_miterlimit,
            &src.values.stroke_miterlimit,
        );
        inherit(
            inherit_fn,
            &mut self.values.stroke_width,
            &src.values.stroke_width,
        );
        inherit(
            inherit_fn,
            &mut self.values.text_anchor,
            &src.values.text_anchor,
        );
        inherit(
            inherit_fn,
            &mut self.values.text_decoration,
            &src.values.text_decoration,
        );
        inherit(
            inherit_fn,
            &mut self.values.text_rendering,
            &src.values.text_rendering,
        );
        inherit(
            inherit_fn,
            &mut self.values.unicode_bidi,
            &src.values.unicode_bidi,
        );
        inherit(
            inherit_fn,
            &mut self.values.visibility,
            &src.values.visibility,
        );
        inherit(inherit_fn, &mut self.values.xml_lang, &src.values.xml_lang);
        inherit(
            inherit_fn,
            &mut self.values.xml_space,
            &src.values.xml_space,
        );

        self.cond = src.cond;

        if inherituninheritables {
            self.values.clip_path.clone_from(&src.values.clip_path);
            self.values.comp_op.clone_from(&src.values.comp_op);
            self.values
                .enable_background
                .clone_from(&src.values.enable_background);
            self.values.filter.clone_from(&src.values.filter);
            self.values.mask.clone_from(&src.values.mask);
            self.values.opacity.clone_from(&src.values.opacity);
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

    pub fn parse_presentation_attributes(&mut self, pbag: &PropertyBag) -> Result<(), NodeError> {
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
        values.affine = cairo::Matrix::multiply(&self.affine, &values.affine);
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
    T: Property + Parse + Default + Clone,
{
    if value.trim() == "inherit" {
        Ok(SpecifiedValue::Inherit)
    } else {
        Parse::parse(value, data).map(SpecifiedValue::Specified)
    }
}

make_property!(
    BaselineShift,
    default: 0f64,
    inherits_automatically: true,
    newtype: f64
);

impl Parse for BaselineShift {
    type Data = ();
    type Err = AttributeError;

    // These values come from Inkscape's SP_CSS_BASELINE_SHIFT_(SUB/SUPER/BASELINE);
    // see sp_style_merge_baseline_shift_from_parent()
    fn parse(s: &str, _: Self::Data) -> Result<BaselineShift, ::error::AttributeError> {
        match s.trim() {
            "baseline" => Ok(BaselineShift(0f64)),
            "sub" => Ok(BaselineShift(-0.2f64)),
            "super" => Ok(BaselineShift(0.4f64)),

            _ => Err(::error::AttributeError::from(::parsers::ParseError::new(
                "invalid value",
            ))),
        }
    }
}

make_property!(
    ClipPath,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
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
    Color,
    default: cssparser::RGBA::new(0, 0, 0, 0xff),
    inherits_automatically: true,
    newtype_parse: cssparser::RGBA,
    parse_data_type: ()
);

make_property!(
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
    Direction,
    default: Ltr,
    inherits_automatically: true,

    identifiers:
    "ltr" => Ltr,
    "rtl" => Rtl,
);

make_property!(
    Display,
    default: Inline,
    inherits_automatically: true,

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
    EnableBackground,
    default: Accumulate,
    inherits_automatically: false,

    identifiers:
    "accumulate" => Accumulate,
    "new" => New,
);

make_property!(
    Fill,
    default: PaintServer::parse("#000", ()).unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

make_property!(
    FillOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    FillRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

make_property!(
    Filter,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/filters.html#FloodColorProperty
make_property!(
    FloodColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

// https://www.w3.org/TR/SVG/filters.html#FloodOpacityProperty
make_property!(
    FloodOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_from_str: UnitInterval
);

make_property!(
    FontFamily,
    default: "Times New Roman".to_string(),
    inherits_automatically: true,
    newtype_from_str: String
);

make_property!(
    FontSize,
    default: RsvgLength::parse("12.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

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

make_property!(
    FontStyle,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "italic" => Italic,
    "oblique" => Oblique,
);

make_property!(
    FontVariant,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "small-caps" => SmallCaps,
);

make_property!(
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
    LetterSpacing,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

// https://www.w3.org/TR/SVG/filters.html#LightingColorProperty
make_property!(
    LightingColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(255, 255, 255, 255)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

make_property!(
    MarkerEnd,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    MarkerMid,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    MarkerStart,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    Mask,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    Opacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_from_str: UnitInterval
);

make_property!(
    Overflow,
    default: Visible,
    inherits_automatically: true,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "scroll" => Scroll,
    "auto" => Auto,
);

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

make_property!(
    StopColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

make_property!(
    StopOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_from_str: UnitInterval
);

make_property!(
    Stroke,
    default: PaintServer::None,
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

make_property!(
    StrokeDasharray,
    default: Dasharray::default(),
    inherits_automatically: true,
    newtype_parse: Dasharray,
    parse_data_type: ()
);

make_property!(
    StrokeDashoffset,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

make_property!(
    StrokeLinecap,
    default: Butt,
    inherits_automatically: true,

    identifiers:
    "butt" => Butt,
    "round" => Round,
    "square" => Square,
);

make_property!(
    StrokeLinejoin,
    default: Miter,
    inherits_automatically: true,

    identifiers:
    "miter" => Miter,
    "round" => Round,
    "bevel" => Bevel,
);

make_property!(
    StrokeOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    StrokeMiterlimit,
    default: 4f64,
    inherits_automatically: true,
    newtype_from_str: f64
);

make_property!(
    StrokeWidth,
    default: RsvgLength::parse("1.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

make_property!(
    TextAnchor,
    default: Start,
    inherits_automatically: true,

    identifiers:
    "start" => Start,
    "middle" => Middle,
    "end" => End,
);

make_property!(
    TextDecoration,
    inherits_automatically: true,

    fields:
    overline: bool, default: false,
    underline: bool, default: false,
    strike: bool, default: false,
);

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

make_property!(
    UnicodeBidi,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "embed" => Embed,
    "bidi-override" => Override,
);

make_property!(
    Visibility,
    default: Visible,
    inherits_automatically: true,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "collapse" => Collapse,
);

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

make_property!(
    XmlLang,
    default: "".to_string(), // see create_pango_layout()
    inherits_automatically: true,
    newtype_from_str: String
);

make_property!(
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

pub fn to_c(state: &State) -> *const RsvgState {
    state as *const State as *const RsvgState
}

pub fn to_c_mut(state: &mut State) -> *mut RsvgState {
    state as *mut State as *mut RsvgState
}

// Rust State API for consumption from C ----------------------------------------

#[no_mangle]
pub extern "C" fn rsvg_state_new(parent: *mut RsvgState) -> *mut RsvgState {
    Box::into_raw(Box::new(State::new(parent))) as *mut RsvgState
}

#[no_mangle]
pub extern "C" fn rsvg_state_free(state: *mut RsvgState) {
    let state = from_c_mut(state);

    unsafe {
        Box::from_raw(state);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_parent(state: *const RsvgState) -> *mut RsvgState {
    let state = from_c(state);

    state.parent as *mut _
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

fn inherit<T>(
    inherit_fn: fn(bool, bool) -> bool,
    dst: &mut SpecifiedValue<T>,
    src: &SpecifiedValue<T>,
) where
    T: Property + Clone + Default,
{
    let dst_has_val = if let SpecifiedValue::Specified(_) = *dst {
        true
    } else {
        false
    };

    let src_has_val = if let SpecifiedValue::Specified(_) = *src {
        true
    } else {
        false
    };

    if inherit_fn(dst_has_val, src_has_val) {
        dst.clone_from(src);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_affine(state: *const RsvgState) -> cairo::Matrix {
    let state = from_c(state);

    state.affine
}

#[no_mangle]
pub extern "C" fn rsvg_state_set_affine(state: *mut RsvgState, affine: cairo::Matrix) {
    let state = from_c_mut(state);
    state.affine = affine;
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

                Attribute::Transform => match cairo::Matrix::parse(value, ()) {
                    Ok(affine) => state.affine = cairo::Matrix::multiply(&affine, &state.affine),

                    Err(e) => {
                        node.set_error(NodeError::attribute_error(Attribute::Transform, e));
                        break;
                    }
                },

                _ => (),
            }
        }
    }
}
