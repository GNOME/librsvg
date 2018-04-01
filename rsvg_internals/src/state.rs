use cairo::{self, MatrixTrait};
use glib::translate::*;
use glib_sys;
use libc;
use pango;
use pango_sys;

use attributes::Attribute;
use color::{Color, ColorSpec};
use error::*;
use length::{LengthDir, RsvgLength, StrokeDasharray};
use node::RsvgNode;
use opacity::{Opacity, OpacitySpec};
use paint_server::PaintServer;
use parsers::Parse;
use property_macros::Property;
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
    pub cap: Option<StrokeLinecap>,
    pub fill_rule: Option<FillRule>,
    pub font_family: Option<FontFamily>,
    pub font_size: Option<FontSize>,
    pub join: Option<StrokeLinejoin>,
    pub letter_spacing: Option<LetterSpacing>,
    pub overflow: Option<Overflow>,
    pub text_anchor: Option<TextAnchor>,
    pub text_decoration: Option<TextDecoration>,
    pub xml_lang: Option<XmlLang>,
    pub xml_space: Option<XmlSpace>,
}

impl State {
    fn new() -> State {
        State {
            affine: cairo::Matrix::identity(),

            // please keep these sorted
            baseline_shift: Default::default(),
            cap: Default::default(),
            fill_rule: Default::default(),
            font_family: Default::default(),
            font_size: Default::default(),
            join: Default::default(),
            letter_spacing: Default::default(),
            overflow: Default::default(),
            text_anchor: Default::default(),
            text_decoration: Default::default(),
            xml_lang: Default::default(),
            xml_space: Default::default(),
        }
    }

    fn parse_style_pair(&mut self, attr: Attribute, value: &str) -> Result<(), AttributeError> {
        // please keep these sorted
        match attr {
            Attribute::BaselineShift => {
                self.baseline_shift = parse_property(value, ())?;
            }

            Attribute::FillRule => {
                self.fill_rule = parse_property(value, ())?;
            }

            Attribute::FontFamily => {
                self.font_family = parse_property(value, ())?;
            }

            Attribute::FontSize => {
                self.font_size = parse_property(value, LengthDir::Both)?;
            }

            Attribute::StrokeLinecap => {
                self.cap = parse_property(value, ())?;
            }

            Attribute::StrokeLinejoin => {
                self.join = parse_property(value, ())?;
            }

            Attribute::LetterSpacing => {
                self.letter_spacing = parse_property(value, LengthDir::Horizontal)?;
            }

            Attribute::Overflow => {
                self.overflow = parse_property(value, ())?;
            }

            Attribute::TextAnchor => {
                self.text_anchor = parse_property(value, ())?;
            }

            Attribute::TextDecoration => {
                self.text_decoration = parse_property(value, ())?;
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

// Keep in sync with rsvg-styles.h:UnicodeBidi
// FIXME: these are not constructed in the Rust code yet, but they are in C.  Remove this
// when that code is moved to Rust.
#[allow(dead_code)]
#[repr(C)]
pub enum UnicodeBidi {
    Normal,
    Embed,
    Override,
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_state_new() -> *mut RsvgState;
    fn rsvg_state_new_with_parent(parent: *mut RsvgState) -> *mut RsvgState;
    fn rsvg_state_free(state: *mut RsvgState);
    fn rsvg_state_reinit(state: *mut RsvgState);
    fn rsvg_state_clone(state: *mut RsvgState, src: *const RsvgState);
    fn rsvg_state_parent(state: *const RsvgState) -> *mut RsvgState;
    fn rsvg_state_get_cond_true(state: *const RsvgState) -> glib_sys::gboolean;
    fn rsvg_state_set_cond_true(state: *const RsvgState, cond_true: glib_sys::gboolean);
    fn rsvg_state_get_stop_color(state: *const RsvgState) -> *const ColorSpec;
    fn rsvg_state_get_stop_opacity(state: *const RsvgState) -> *const OpacitySpec;
    fn rsvg_state_get_stroke_dasharray(state: *const RsvgState) -> *const StrokeDasharray;
    fn rsvg_state_get_dash_offset(state: *const RsvgState) -> RsvgLength;
    fn rsvg_state_get_current_color(state: *const RsvgState) -> u32;
    fn rsvg_state_get_shape_rendering_type(state: *const RsvgState) -> cairo::Antialias;
    fn rsvg_state_get_text_rendering_type(state: *const RsvgState) -> cairo::Antialias;
    fn rsvg_state_get_stroke(state: *const RsvgState) -> *const PaintServer;
    fn rsvg_state_get_stroke_opacity(state: *const RsvgState) -> u8;
    fn rsvg_state_get_stroke_width(state: *const RsvgState) -> RsvgLength;
    fn rsvg_state_get_miter_limit(state: *const RsvgState) -> f64;
    fn rsvg_state_get_unicode_bidi(state: *const RsvgState) -> UnicodeBidi;
    fn rsvg_state_get_text_dir(state: *const RsvgState) -> pango_sys::PangoDirection;
    fn rsvg_state_get_text_gravity(state: *const RsvgState) -> pango_sys::PangoGravity;
    fn rsvg_state_get_font_style(state: *const RsvgState) -> pango_sys::PangoStyle;
    fn rsvg_state_get_font_variant(state: *const RsvgState) -> pango_sys::PangoVariant;
    fn rsvg_state_get_font_weight(state: *const RsvgState) -> pango_sys::PangoWeight;
    fn rsvg_state_get_font_stretch(state: *const RsvgState) -> pango_sys::PangoStretch;
    fn rsvg_state_get_clip_rule(state: *const RsvgState) -> cairo::FillRule;
    fn rsvg_state_get_fill(state: *const RsvgState) -> *const PaintServer;
    fn rsvg_state_get_fill_opacity(state: *const RsvgState) -> u8;
    fn rsvg_state_get_comp_op(state: *const RsvgState) -> cairo::Operator;

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
        _ => false
    }
}

pub fn get_cond_true(state: *const RsvgState) -> bool {
    unsafe { from_glib(rsvg_state_get_cond_true(state)) }
}

pub fn set_cond_true(state: *const RsvgState, cond_true: bool) {
    unsafe {
        rsvg_state_set_cond_true(state, cond_true.to_glib());
    }
}

pub fn get_stop_color(state: *const RsvgState) -> Result<Option<Color>, AttributeError> {
    unsafe {
        let spec_ptr = rsvg_state_get_stop_color(state);

        if spec_ptr.is_null() {
            Ok(None)
        } else {
            Color::from_color_spec(&*spec_ptr).map(Some)
        }
    }
}

pub fn get_stop_opacity(state: *const RsvgState) -> Result<Option<Opacity>, AttributeError> {
    unsafe {
        let opacity_ptr = rsvg_state_get_stop_opacity(state);

        if opacity_ptr.is_null() {
            Ok(None)
        } else {
            Opacity::from_opacity_spec(&*opacity_ptr).map(Some)
        }
    }
}

pub fn get_stroke_dasharray<'a>(state: *const RsvgState) -> Option<&'a StrokeDasharray> {
    let dash = unsafe { rsvg_state_get_stroke_dasharray(state) };

    if dash.is_null() {
        None
    } else {
        unsafe { Some(&*dash) }
    }
}

pub fn get_dash_offset(state: *const RsvgState) -> RsvgLength {
    unsafe { rsvg_state_get_dash_offset(state) }
}

pub fn get_current_color(state: *const RsvgState) -> Color {
    let argb = unsafe { rsvg_state_get_current_color(state) };

    Color::from(argb)
}

pub fn get_shape_rendering_type(state: *const RsvgState) -> cairo::Antialias {
    unsafe { rsvg_state_get_shape_rendering_type(state) }
}

pub fn get_text_rendering_type(state: *const RsvgState) -> cairo::Antialias {
    unsafe { rsvg_state_get_text_rendering_type(state) }
}

pub fn get_stroke<'a>(state: *const RsvgState) -> Option<&'a PaintServer> {
    unsafe {
        let ps = rsvg_state_get_stroke(state);

        if ps.is_null() {
            None
        } else {
            Some(&*ps)
        }
    }
}

pub fn get_stroke_opacity(state: *const RsvgState) -> u8 {
    unsafe { rsvg_state_get_stroke_opacity(state) }
}

pub fn get_stroke_width(state: *const RsvgState) -> RsvgLength {
    unsafe { rsvg_state_get_stroke_width(state) }
}

pub fn get_miter_limit(state: *const RsvgState) -> f64 {
    unsafe { rsvg_state_get_miter_limit(state) }
}

pub fn get_unicode_bidi(state: *const RsvgState) -> UnicodeBidi {
    unsafe { rsvg_state_get_unicode_bidi(state) }
}

pub fn get_text_dir(state: *const RsvgState) -> pango::Direction {
    unsafe { from_glib(rsvg_state_get_text_dir(state)) }
}

pub fn get_text_gravity(state: *const RsvgState) -> pango::Gravity {
    unsafe { from_glib(rsvg_state_get_text_gravity(state)) }
}

pub fn get_font_style(state: *const RsvgState) -> pango::Style {
    unsafe { from_glib(rsvg_state_get_font_style(state)) }
}

pub fn get_font_variant(state: *const RsvgState) -> pango::Variant {
    unsafe { from_glib(rsvg_state_get_font_variant(state)) }
}

pub fn get_font_weight(state: *const RsvgState) -> pango::Weight {
    unsafe { from_glib(rsvg_state_get_font_weight(state)) }
}

pub fn get_font_stretch(state: *const RsvgState) -> pango::Stretch {
    unsafe { from_glib(rsvg_state_get_font_stretch(state)) }
}

pub fn get_clip_rule(state: *const RsvgState) -> cairo::FillRule {
    unsafe { rsvg_state_get_clip_rule(state) }
}

pub fn get_fill<'a>(state: *const RsvgState) -> Option<&'a PaintServer> {
    unsafe {
        let ps = rsvg_state_get_fill(state);

        if ps.is_null() {
            None
        } else {
            Some(&*ps)
        }
    }
}

pub fn get_fill_opacity(state: *const RsvgState) -> u8 {
    unsafe { rsvg_state_get_fill_opacity(state) }
}

pub fn get_comp_op(state: *const RsvgState) -> cairo::Operator {
    unsafe { rsvg_state_get_comp_op(state) }
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

pub fn get_state_rust<'a>(state: *const RsvgState) -> &'a mut State {
    unsafe { &mut *rsvg_state_get_state_rust(state) }
}

// BaselineShift -----------------------------------

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

// FillRule ----------------------------------------

make_property!(
    FillRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

// FontFamily -----------------------------------------

make_property!(
    FontFamily,
    default: "Times New Roman".to_string(),
    inherits_automatically: true,
    newtype_from_str: String
);

// FontSize -------------------------------------------

make_property!(
    FontSize,
    default: RsvgLength::parse("12.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype: RsvgLength
);

impl Parse for FontSize {
    type Data = LengthDir;
    type Err = AttributeError;

    fn parse(s: &str, dir: LengthDir) -> Result<FontSize, AttributeError> {
        Ok(FontSize(RsvgLength::parse(s, dir)?))
    }
}

// StrokeLinecap ----------------------------------------

make_property!(
    StrokeLinecap,
    default: Butt,
    inherits_automatically: true,

    identifiers:
    "butt" => Butt,
    "round" => Round,
    "square" => Square,
);

// StrokeLineJoin ----------------------------------------

make_property!(
    StrokeLinejoin,
    default: Miter,
    inherits_automatically: true,

    identifiers:
    "miter" => Miter,
    "round" => Round,
    "bevel" => Bevel,
);

// LetterSpacing -----------------------------------

make_property!(
    LetterSpacing,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype: RsvgLength
);

impl Parse for LetterSpacing {
    type Data = LengthDir;
    type Err = AttributeError;

    fn parse(s: &str, dir: LengthDir) -> Result<LetterSpacing, AttributeError> {
        Ok(LetterSpacing(RsvgLength::parse(s, dir)?))
    }
}

// Overflow ----------------------------------------

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

// TextDecoration ----------------------------------

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

// TextAnchor --------------------------------------

make_property!(
    TextAnchor,
    default: Start,
    inherits_automatically: true,

    identifiers:
    "start" => Start,
    "middle" => Middle,
    "end" => End,
);

// XmlLang ----------------------------------------

make_property!(
    XmlLang,
    default: "C".to_string(),
    inherits_automatically: true,
    newtype_from_str: String
);

// XmlSpace ----------------------------------------

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
pub extern "C" fn rsvg_state_rust_parse_style_pair(
    state: *mut State,
    attr: Attribute,
    value: *const libc::c_char,
) -> glib_sys::gboolean {
    assert!(!state.is_null());
    assert!(!value.is_null());

    let state = unsafe { &mut *state };
    let value = unsafe { utf8_cstr(value) };

    match state.parse_style_pair(attr, value) {
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
) {
    assert!(!dst.is_null());
    assert!(!src.is_null());

    let dst = unsafe { &mut *dst };
    let src = unsafe { &*src };

    // please keep these sorted
    inherit(inherit_fn, &mut dst.baseline_shift, &src.baseline_shift);
    inherit(inherit_fn, &mut dst.cap, &src.cap);
    inherit(inherit_fn, &mut dst.fill_rule, &src.fill_rule);
    inherit(inherit_fn, &mut dst.font_family, &src.font_family);
    inherit(inherit_fn, &mut dst.font_size, &src.font_size);
    inherit(inherit_fn, &mut dst.join, &src.join);
    inherit(inherit_fn, &mut dst.letter_spacing, &src.letter_spacing);
    inherit(inherit_fn, &mut dst.overflow, &src.overflow);
    inherit(inherit_fn, &mut dst.text_anchor, &src.text_anchor);
    inherit(inherit_fn, &mut dst.text_decoration, &src.text_decoration);
    inherit(inherit_fn, &mut dst.xml_lang, &src.xml_lang);
    inherit(inherit_fn, &mut dst.xml_space, &src.xml_space);
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
