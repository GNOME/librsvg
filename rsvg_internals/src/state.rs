use cairo::{self, MatrixTrait};
use cssparser;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr;

use attributes::Attribute;
use color::{self, rgba_to_argb};
use cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use error::*;
use iri::IRI;
use length::{Dasharray, LengthDir, RsvgLength};
use node::RsvgNode;
use opacity;
use paint_server::PaintServer;
use parsers::Parse;
use property_bag::PropertyBag;
use property_macros::Property;
use unitinterval::UnitInterval;
use util::utf8_cstr;

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
#[derive(Clone)]
pub struct State {
    pub affine: cairo::Matrix,

    pub baseline_shift: Option<BaselineShift>,
    pub clip_path: Option<ClipPath>,
    pub clip_rule: Option<ClipRule>,
    pub comp_op: Option<CompOp>,
    pub color: Option<Color>,
    pub direction: Option<Direction>,
    pub display: Option<Display>,
    pub enable_background: Option<EnableBackground>,
    pub fill: Option<Fill>,
    pub fill_opacity: Option<FillOpacity>,
    pub fill_rule: Option<FillRule>,
    pub filter: Option<Filter>,
    pub flood_color: Option<FloodColor>,
    pub flood_opacity: Option<FloodOpacity>,
    pub font_family: Option<FontFamily>,
    pub font_size: Option<FontSize>,
    pub font_stretch: Option<FontStretch>,
    pub font_style: Option<FontStyle>,
    pub font_variant: Option<FontVariant>,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<LetterSpacing>,
    pub marker_end: Option<MarkerEnd>,
    pub marker_mid: Option<MarkerMid>,
    pub marker_start: Option<MarkerStart>,
    pub mask: Option<Mask>,
    pub opacity: Option<Opacity>,
    pub overflow: Option<Overflow>,
    pub shape_rendering: Option<ShapeRendering>,
    pub stroke: Option<Stroke>,
    pub stroke_dasharray: Option<StrokeDasharray>,
    pub stroke_dashoffset: Option<StrokeDashoffset>,
    pub stroke_line_cap: Option<StrokeLinecap>,
    pub stroke_line_join: Option<StrokeLinejoin>,
    pub stroke_opacity: Option<StrokeOpacity>,
    pub stroke_miterlimit: Option<StrokeMiterlimit>,
    pub stroke_width: Option<StrokeWidth>,
    pub text_anchor: Option<TextAnchor>,
    pub text_decoration: Option<TextDecoration>,
    pub text_rendering: Option<TextRendering>,
    pub unicode_bidi: Option<UnicodeBidi>,
    pub visibility: Option<Visibility>,
    pub writing_mode: Option<WritingMode>,
    pub xml_lang: Option<XmlLang>,
    pub xml_space: Option<XmlSpace>,

    important_styles: RefCell<HashSet<Attribute>>,
    cond: bool,
}

impl State {
    fn new() -> State {
        State {
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
            marker_end: Default::default(),
            marker_mid: Default::default(),
            marker_start: Default::default(),
            mask: Default::default(),
            opacity: Default::default(),
            overflow: Default::default(),
            shape_rendering: Default::default(),
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

            important_styles: Default::default(),
            cond: true,
        }
    }

    fn parse_style_pair(
        &mut self,
        attr: Attribute,
        value: &str,
        accept_shorthands: bool,
    ) -> Result<(), AttributeError> {
        // please keep these sorted
        match attr {
            Attribute::BaselineShift => {
                self.baseline_shift = parse_property(value, ())?;
            }

            Attribute::ClipPath => {
                self.clip_path = parse_property(value, ())?;
            }

            Attribute::ClipRule => {
                self.clip_rule = parse_property(value, ())?;
            }

            Attribute::Color => {
                self.color = parse_property(value, ())?;
            }

            Attribute::CompOp => {
                self.comp_op = parse_property(value, ())?;
            }

            Attribute::Direction => {
                self.direction = parse_property(value, ())?;
            }

            Attribute::Display => {
                self.display = parse_property(value, ())?;
            }

            Attribute::EnableBackground => {
                self.enable_background = parse_property(value, ())?;
            }

            Attribute::Fill => {
                self.fill = parse_property(value, ())?;
            }

            Attribute::FillOpacity => {
                self.fill_opacity = parse_property(value, ())?;
            }

            Attribute::FillRule => {
                self.fill_rule = parse_property(value, ())?;
            }

            Attribute::Filter => {
                self.filter = parse_property(value, ())?;
            }

            Attribute::FloodColor => {
                self.flood_color = parse_property(value, ())?;
            }

            Attribute::FloodOpacity => {
                self.flood_opacity = parse_property(value, ())?;
            }

            Attribute::FontFamily => {
                self.font_family = parse_property(value, ())?;
            }

            Attribute::FontSize => {
                self.font_size = parse_property(value, LengthDir::Both)?;
            }

            Attribute::FontStretch => {
                self.font_stretch = parse_property(value, ())?;
            }

            Attribute::FontStyle => {
                self.font_style = parse_property(value, ())?;
            }

            Attribute::FontVariant => {
                self.font_variant = parse_property(value, ())?;
            }

            Attribute::FontWeight => {
                self.font_weight = parse_property(value, ())?;
            }

            Attribute::LetterSpacing => {
                self.letter_spacing = parse_property(value, LengthDir::Horizontal)?;
            }

            Attribute::MarkerEnd => {
                self.marker_end = parse_property(value, ())?;
            }

            Attribute::MarkerMid => {
                self.marker_mid = parse_property(value, ())?;
            }

            Attribute::MarkerStart => {
                self.marker_start = parse_property(value, ())?;
            }

            Attribute::Marker if accept_shorthands => {
                if self.marker_end.is_none() {
                    self.marker_end = parse_property(value, ())?;
                }

                if self.marker_mid.is_none() {
                    self.marker_mid = parse_property(value, ())?;
                }

                if self.marker_start.is_none() {
                    self.marker_start = parse_property(value, ())?;
                }
            }

            Attribute::Mask => {
                self.mask = parse_property(value, ())?;
            }

            Attribute::Opacity => {
                self.opacity = parse_property(value, ())?;
            }

            Attribute::Overflow => {
                self.overflow = parse_property(value, ())?;
            }

            Attribute::ShapeRendering => {
                self.shape_rendering = parse_property(value, ())?;
            }

            Attribute::Stroke => {
                self.stroke = parse_property(value, ())?;
            }

            Attribute::StrokeDasharray => {
                self.stroke_dasharray = parse_property(value, ())?;
            }

            Attribute::StrokeDashoffset => {
                self.stroke_dashoffset = parse_property(value, LengthDir::Both)?;
            }

            Attribute::StrokeLinecap => {
                self.stroke_line_cap = parse_property(value, ())?;
            }

            Attribute::StrokeLinejoin => {
                self.stroke_line_join = parse_property(value, ())?;
            }

            Attribute::StrokeOpacity => {
                self.stroke_opacity = parse_property(value, ())?;
            }

            Attribute::StrokeMiterlimit => {
                self.stroke_miterlimit = parse_property(value, ())?;
            }

            Attribute::StrokeWidth => {
                self.stroke_width = parse_property(value, LengthDir::Both)?;
            }

            Attribute::TextAnchor => {
                self.text_anchor = parse_property(value, ())?;
            }

            Attribute::TextDecoration => {
                self.text_decoration = parse_property(value, ())?;
            }

            Attribute::TextRendering => {
                self.text_rendering = parse_property(value, ())?;
            }

            Attribute::UnicodeBidi => {
                self.unicode_bidi = parse_property(value, ())?;
            }

            Attribute::Visibility => {
                self.visibility = parse_property(value, ())?;
            }

            Attribute::WritingMode => {
                self.writing_mode = parse_property(value, ())?;
            }

            Attribute::XmlLang => {
                // xml:lang is not a property; it is a non-presentation attribute and as such
                // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                // but rather call its parser directly.
                self.xml_lang = Some(XmlLang::parse(value, ())?);
            }

            Attribute::XmlSpace => {
                // xml:space is not a property; it is a non-presentation attribute and as such
                // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                // but rather call its parser directly.
                self.xml_space = Some(XmlSpace::parse(value, ())?);
            }

            _ => {
                // Maybe it's an attribute not parsed here, but in the
                // node implementations.
            }
        }

        Ok(())
    }

    pub fn parse_conditional_processing_attributes(
        &mut self,
        pbag: &PropertyBag,
    ) -> Result<(), AttributeError> {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::RequiredExtensions if self.cond => {
                    self.cond =
                        RequiredExtensions::parse(value, ()).map(|RequiredExtensions(res)| res)?;
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
        }

        Ok(())
    }
}

// Parses the `value` for the type `T` of the property, including `inherit` values.
//
// If the `value` is `inherit`, returns `Ok(None)`; otherwise returns
// `Ok(Some(T))`.
fn parse_property<T>(value: &str, data: <T as Parse>::Data) -> Result<Option<T>, <T as Parse>::Err>
where
    T: Property + Parse,
{
    if value.trim() == "inherit" {
        Ok(None)
    } else {
        Parse::parse(value, data).map(Some)
    }
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_state_new() -> *mut RsvgState;
    fn rsvg_state_new_with_parent(parent: *mut RsvgState) -> *mut RsvgState;
    fn rsvg_state_free(state: *mut RsvgState);
    fn rsvg_state_reinit(state: *mut RsvgState);
    fn rsvg_state_clone(state: *mut RsvgState, src: *const RsvgState);
    fn rsvg_state_parent(state: *const RsvgState) -> *mut RsvgState;
    fn rsvg_state_get_stop_color(state: *const RsvgState) -> *const color::ColorSpec;
    fn rsvg_state_get_stop_opacity(state: *const RsvgState) -> *const opacity::OpacitySpec;

    fn rsvg_state_dominate(state: *mut RsvgState, src: *const RsvgState);
    fn rsvg_state_force(state: *mut RsvgState, src: *const RsvgState);
    fn rsvg_state_inherit(state: *mut RsvgState, src: *const RsvgState);
    fn rsvg_state_reinherit(state: *mut RsvgState, src: *const RsvgState);

    fn rsvg_state_get_state_rust(state: *const RsvgState) -> *mut State;
}

pub fn new() -> *mut RsvgState {
    unsafe { rsvg_state_new() }
}

pub fn new_with_parent(parent: *mut RsvgState) -> *mut RsvgState {
    unsafe { rsvg_state_new_with_parent(parent) }
}

pub fn free(state: *mut RsvgState) {
    unsafe {
        rsvg_state_free(state);
    }
}

pub fn reinit(state: *mut RsvgState) {
    unsafe {
        rsvg_state_reinit(state);
    }
}

pub fn reconstruct(state: *mut RsvgState, node: &RsvgNode) {
    if let Some(parent) = node.get_parent() {
        reconstruct(state, &parent);
        unsafe {
            rsvg_state_inherit(state, node.get_state());
        }
    }
}

pub fn clone_from(state: *mut RsvgState, src: *const RsvgState) {
    unsafe {
        rsvg_state_clone(state, src);
    }
}

pub fn parent(state: *const RsvgState) -> Option<*mut RsvgState> {
    let parent = unsafe { rsvg_state_parent(state) };

    if parent.is_null() {
        None
    } else {
        Some(parent)
    }
}

pub fn is_overflow(state: *const RsvgState) -> bool {
    let rstate = get_state_rust(state);

    match rstate.overflow {
        Some(Overflow::Auto) | Some(Overflow::Visible) => true,
        _ => false,
    }
}

pub fn is_visible(state: *const RsvgState) -> bool {
    let rstate = get_state_rust(state);

    match (rstate.display, rstate.visibility) {
        (Some(Display::None), _) => false,
        (_, None) | (_, Some(Visibility::Visible)) => true,
        _ => false,
    }
}

pub fn text_gravity_is_vertical(state: *const RsvgState) -> bool {
    let rstate = get_state_rust(state);

    match rstate.writing_mode {
        Some(WritingMode::Tb) | Some(WritingMode::TbRl) => true,
        _ => false,
    }
}

pub fn get_stop_color(state: *const RsvgState) -> Result<Option<color::Color>, AttributeError> {
    unsafe {
        let spec_ptr = rsvg_state_get_stop_color(state);

        if spec_ptr.is_null() {
            Ok(None)
        } else {
            color::from_color_spec(&*spec_ptr)
        }
    }
}

pub fn get_stop_opacity(
    state: *const RsvgState,
) -> Result<Option<opacity::Opacity>, AttributeError> {
    unsafe {
        let opacity_ptr = rsvg_state_get_stop_opacity(state);

        if opacity_ptr.is_null() {
            Ok(None)
        } else {
            opacity::Opacity::from_opacity_spec(&*opacity_ptr).map(Some)
        }
    }
}

pub fn dominate(state: *mut RsvgState, src: *const RsvgState) {
    unsafe {
        rsvg_state_dominate(state, src);
    }
}

pub fn force(state: *mut RsvgState, src: *const RsvgState) {
    unsafe {
        rsvg_state_force(state, src);
    }
}

pub fn reinherit(state: *mut RsvgState, src: *const RsvgState) {
    unsafe {
        rsvg_state_reinherit(state, src);
    }
}

pub fn get_cond(state: *mut RsvgState) -> bool {
    get_state_rust(state).cond
}

pub fn set_cond(state: *mut RsvgState, value: bool) {
    let rstate = get_state_rust(state);

    rstate.cond = value;
}

pub fn get_state_rust<'a>(state: *const RsvgState) -> &'a mut State {
    unsafe { &mut *rsvg_state_get_state_rust(state) }
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

make_property!(
    FloodColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: true,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

make_property!(
    FloodOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
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
    Stroke,
    default: PaintServer::parse("#000", ()).unwrap(),
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
    default: "C".to_string(),
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

// C state API implemented in rust

#[no_mangle]
pub extern "C" fn rsvg_state_reconstruct(state: *mut RsvgState, raw_node: *const RsvgNode) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    reconstruct(state, node);
}

#[no_mangle]
pub extern "C" fn rsvg_state_is_visible(state: *const RsvgState) -> glib_sys::gboolean {
    is_visible(state).to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_state_parse_conditional_processing_attributes(
    state: *mut RsvgState,
    pbag: *const PropertyBag,
) -> glib_sys::gboolean {
    let state = unsafe { &mut *state };
    let pbag = unsafe { &*pbag };

    let rstate = get_state_rust(state);
    match rstate.parse_conditional_processing_attributes(pbag) {
        Ok(_) => true.to_glib(),
        Err(_) => false.to_glib(),
    }
}

// Rust State API for consumption from C ----------------------------------------

#[no_mangle]
pub extern "C" fn rsvg_state_rust_new() -> *mut State {
    Box::into_raw(Box::new(State::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_free(state: *mut State) {
    assert!(!state.is_null());

    unsafe {
        Box::from_raw(state);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_clone(state: *const State) -> *mut State {
    assert!(!state.is_null());

    unsafe { Box::into_raw(Box::new((*state).clone())) }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_contains_important_style(
    state: *const State,
    attr: Attribute,
) -> glib_sys::gboolean {
    let state = unsafe { &*state };

    state.important_styles.borrow().contains(&attr).to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_insert_important_style(state: *mut State, attr: Attribute) {
    let state = unsafe { &mut *state };

    state.important_styles.borrow_mut().insert(attr);
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_parse_style_pair(
    state: *mut State,
    attr: Attribute,
    value: *const libc::c_char,
    accept_shorthands: glib_sys::gboolean,
) -> glib_sys::gboolean {
    assert!(!state.is_null());
    assert!(!value.is_null());

    let state = unsafe { &mut *state };
    let value = unsafe { utf8_cstr(value) };

    match state.parse_style_pair(attr, value, from_glib(accept_shorthands)) {
        Ok(_) => true.to_glib(),
        Err(_) => false.to_glib(),
    }
}

fn should_inherit_from_src(
    inherit_fn: extern "C" fn(glib_sys::gboolean, glib_sys::gboolean) -> glib_sys::gboolean,
    dst: bool,
    src: bool,
) -> bool {
    from_glib(inherit_fn(dst.to_glib(), src.to_glib()))
}

fn inherit<T>(
    inherit_fn: extern "C" fn(glib_sys::gboolean, glib_sys::gboolean) -> glib_sys::gboolean,
    dst: &mut Option<T>,
    src: &Option<T>,
) where
    T: Property + Clone,
{
    if should_inherit_from_src(inherit_fn, dst.is_some(), src.is_some()) {
        dst.clone_from(src);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_inherit_run(
    dst: *mut State,
    src: *const State,
    inherit_fn: extern "C" fn(glib_sys::gboolean, glib_sys::gboolean) -> glib_sys::gboolean,
    inheritunheritables: glib_sys::gboolean,
) {
    assert!(!dst.is_null());
    assert!(!src.is_null());

    let dst = unsafe { &mut *dst };
    let src = unsafe { &*src };

    // please keep these sorted
    inherit(inherit_fn, &mut dst.baseline_shift, &src.baseline_shift);
    inherit(inherit_fn, &mut dst.clip_rule, &src.clip_rule);
    inherit(inherit_fn, &mut dst.color, &src.color);
    inherit(inherit_fn, &mut dst.direction, &src.direction);
    inherit(inherit_fn, &mut dst.display, &src.display);
    inherit(inherit_fn, &mut dst.fill, &src.fill);
    inherit(inherit_fn, &mut dst.fill_opacity, &src.fill_opacity);
    inherit(inherit_fn, &mut dst.fill_rule, &src.fill_rule);
    inherit(inherit_fn, &mut dst.flood_color, &src.flood_color);
    inherit(inherit_fn, &mut dst.flood_opacity, &src.flood_opacity);
    inherit(inherit_fn, &mut dst.font_family, &src.font_family);
    inherit(inherit_fn, &mut dst.font_size, &src.font_size);
    inherit(inherit_fn, &mut dst.font_stretch, &src.font_stretch);
    inherit(inherit_fn, &mut dst.font_style, &src.font_style);
    inherit(inherit_fn, &mut dst.font_variant, &src.font_variant);
    inherit(inherit_fn, &mut dst.font_weight, &src.font_weight);
    inherit(inherit_fn, &mut dst.letter_spacing, &src.letter_spacing);
    inherit(inherit_fn, &mut dst.marker_end, &src.marker_end);
    inherit(inherit_fn, &mut dst.marker_mid, &src.marker_mid);
    inherit(inherit_fn, &mut dst.marker_start, &src.marker_start);
    inherit(inherit_fn, &mut dst.overflow, &src.overflow);
    inherit(inherit_fn, &mut dst.shape_rendering, &src.shape_rendering);
    inherit(inherit_fn, &mut dst.stroke, &src.stroke);
    inherit(inherit_fn, &mut dst.stroke_dasharray, &src.stroke_dasharray);
    inherit(
        inherit_fn,
        &mut dst.stroke_dashoffset,
        &src.stroke_dashoffset,
    );
    inherit(inherit_fn, &mut dst.stroke_line_cap, &src.stroke_line_cap);
    inherit(inherit_fn, &mut dst.stroke_line_join, &src.stroke_line_join);
    inherit(inherit_fn, &mut dst.stroke_opacity, &src.stroke_opacity);
    inherit(
        inherit_fn,
        &mut dst.stroke_miterlimit,
        &src.stroke_miterlimit,
    );
    inherit(inherit_fn, &mut dst.stroke_width, &src.stroke_width);
    inherit(inherit_fn, &mut dst.text_anchor, &src.text_anchor);
    inherit(inherit_fn, &mut dst.text_decoration, &src.text_decoration);
    inherit(inherit_fn, &mut dst.text_rendering, &src.text_rendering);
    inherit(inherit_fn, &mut dst.unicode_bidi, &src.unicode_bidi);
    inherit(inherit_fn, &mut dst.visibility, &src.visibility);
    inherit(inherit_fn, &mut dst.xml_lang, &src.xml_lang);
    inherit(inherit_fn, &mut dst.xml_space, &src.xml_space);

    dst.cond = src.cond;

    if from_glib(inheritunheritables) {
        dst.clip_path.clone_from(&src.clip_path);
        dst.comp_op.clone_from(&src.comp_op);
        dst.enable_background.clone_from(&src.enable_background);
        dst.filter.clone_from(&src.filter);
        dst.mask.clone_from(&src.mask);
        dst.opacity.clone_from(&src.opacity);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_affine(state: *const State) -> cairo::Matrix {
    unsafe {
        let state = &*state;
        state.affine
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_set_affine(state: *mut State, affine: cairo::Matrix) {
    unsafe {
        let state = &mut *state;
        state.affine = affine;
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_color(state: *const State) -> u32 {
    unsafe {
        let state = &*state;

        let current_color = state
            .color
            .as_ref()
            .map_or_else(|| Color::default().0, |c| c.0);

        rgba_to_argb(current_color)
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_comp_op(state: *const State) -> cairo::Operator {
    unsafe {
        let state = &*state;
        cairo::Operator::from(state.comp_op.unwrap_or_default())
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_flood_color(state: *const State) -> u32 {
    unsafe {
        let state = &*state;
        match state.flood_color {
            Some(FloodColor(cssparser::Color::RGBA(rgba))) => rgba_to_argb(rgba),
            // FIXME: fallback to current color if Color::inherit and current color is set
            _ => 0xff000000,
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_flood_opacity(state: *const State) -> u8 {
    unsafe {
        let state = &*state;

        u8::from(
            state
                .flood_opacity
                .as_ref()
                .map_or_else(|| FloodOpacity::default().0, |o| o.0),
        )
    }
}

// Keep in sync with rsvg-styles.h:RsvgEnableBackgroundType
#[allow(dead_code)]
#[repr(C)]
pub enum EnableBackgroundC {
    Accumulate,
    New,
}

impl From<EnableBackground> for EnableBackgroundC {
    fn from(e: EnableBackground) -> EnableBackgroundC {
        match e {
            EnableBackground::Accumulate => EnableBackgroundC::Accumulate,
            EnableBackground::New => EnableBackgroundC::New,
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_enable_background(state: *const State) -> EnableBackgroundC {
    unsafe {
        let state = &*state;
        EnableBackgroundC::from(state.enable_background.unwrap_or_default())
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_clip_path(state: *const State) -> *mut libc::c_char {
    unsafe {
        let state = &*state;

        match state.clip_path {
            Some(ClipPath(IRI::Resource(ref p))) => p.to_glib_full(),
            _ => ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_filter(state: *const State) -> *mut libc::c_char {
    unsafe {
        let state = &*state;

        match state.filter {
            Some(Filter(IRI::Resource(ref f))) => f.to_glib_full(),
            _ => ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_mask(state: *const State) -> *mut libc::c_char {
    unsafe {
        let state = &*state;

        match state.mask {
            Some(Mask(IRI::Resource(ref m))) => m.to_glib_full(),
            _ => ptr::null_mut(),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_rust_get_opacity(state: *const State) -> u8 {
    unsafe {
        let state = &*state;

        u8::from(
            state
                .opacity
                .as_ref()
                .map_or_else(|| FloodOpacity::default().0, |o| o.0),
        )
    }
}
