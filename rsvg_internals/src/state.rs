use cairo;
use glib::translate::*;
use glib_sys;
use libc;
use pango;
use pango_sys;

use length::RsvgLength;

pub enum RsvgState {}

// Keep in sync with rsvg-styles.h:UnicodeBidi
#[repr(C)]
pub enum UnicodeBidi {
    Normal,
    Embed,
    Override,
}

// Keep in sync with rsvg-styles.h:TextDecoration
#[repr(C)]
#[derive(Copy, Clone)]
struct TextDecoration {
    overline: glib_sys::gboolean,
    underline: glib_sys::gboolean,
    strike: glib_sys::gboolean,
}

pub struct FontDecor {
    pub overline: bool,
    pub underline: bool,
    pub strike: bool,
}

impl From<TextDecoration> for FontDecor {
    fn from(td: TextDecoration) -> FontDecor {
        FontDecor {
            overline: from_glib(td.overline),
            underline: from_glib(td.underline),
            strike: from_glib(td.strike),
        }
    }
}

extern "C" {
    fn rsvg_state_get_stroke_opacity(state: *const RsvgState) -> u8;
    fn rsvg_state_get_stroke_width(state: *const RsvgState) -> RsvgLength;
    fn rsvg_state_get_miter_limit(state: *const RsvgState) -> f64;
    fn rsvg_state_get_line_cap(state: *const RsvgState) -> cairo::LineCap;
    fn rsvg_state_get_line_join(state: *const RsvgState) -> cairo::LineJoin;
    fn rsvg_state_get_language(state: *const RsvgState) -> *const libc::c_char;
    fn rsvg_state_get_unicode_bidi(state: *const RsvgState) -> UnicodeBidi;
    fn rsvg_state_get_text_dir(state: *const RsvgState) -> pango_sys::PangoDirection;
    fn rsvg_state_get_text_gravity(state: *const RsvgState) -> pango_sys::PangoGravity;
    fn rsvg_state_get_font_family(state: *const RsvgState) -> *const libc::c_char;
    fn rsvg_state_get_font_style(state: *const RsvgState) -> pango_sys::PangoStyle;
    fn rsvg_state_get_font_variant(state: *const RsvgState) -> pango_sys::PangoVariant;
    fn rsvg_state_get_font_weight(state: *const RsvgState) -> pango_sys::PangoWeight;
    fn rsvg_state_get_font_stretch(state: *const RsvgState) -> pango_sys::PangoStretch;
    fn rsvg_state_get_letter_spacing(state: *const RsvgState) -> RsvgLength;
    fn rsvg_state_get_font_decor(state: *const RsvgState) -> *const TextDecoration;
    fn rsvg_state_get_clip_rule(state: *const RsvgState) -> cairo::FillRule;
    fn rsvg_state_get_fill_rule(state: *const RsvgState) -> cairo::FillRule;
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

pub fn get_line_cap(state: *const RsvgState) -> cairo::LineCap {
    unsafe { rsvg_state_get_line_cap(state) }
}

pub fn get_line_join(state: *const RsvgState) -> cairo::LineJoin {
    unsafe { rsvg_state_get_line_join(state) }
}

pub fn get_language(state: *const RsvgState) -> Option<String> {
    unsafe { from_glib_none(rsvg_state_get_language(state)) }
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

pub fn get_font_family(state: *const RsvgState) -> Option<String> {
    unsafe { from_glib_none(rsvg_state_get_font_family(state)) }
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

pub fn get_letter_spacing(state: *const RsvgState) -> RsvgLength {
    unsafe { rsvg_state_get_letter_spacing(state) }
}

pub fn get_font_decor(state: *const RsvgState) -> Option<FontDecor> {
    unsafe {
        let td = rsvg_state_get_font_decor(state);
        if td.is_null() {
            None
        } else {
            Some(FontDecor::from(*td))
        }
    }
}

pub fn get_clip_rule(state: *const RsvgState) -> cairo::FillRule {
    unsafe { rsvg_state_get_clip_rule(state) }
}

pub fn get_fill_rule(state: *const RsvgState) -> cairo::FillRule {
    unsafe { rsvg_state_get_fill_rule(state) }
}
