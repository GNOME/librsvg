use libc;
use glib::translate::*;
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

extern "C" {
    fn rsvg_state_get_language       (state: *const RsvgState) -> *const libc::c_char;
    fn rsvg_state_get_unicode_bidi   (state: *const RsvgState) -> UnicodeBidi;
    fn rsvg_state_get_text_dir       (state: *const RsvgState) -> pango_sys::PangoDirection;
    fn rsvg_state_get_text_gravity   (state: *const RsvgState) -> pango_sys::PangoGravity;
    fn rsvg_state_get_font_family    (state: *const RsvgState) -> *const libc::c_char;
    fn rsvg_state_get_font_style     (state: *const RsvgState) -> pango_sys::PangoStyle;
    fn rsvg_state_get_font_variant   (state: *const RsvgState) -> pango_sys::PangoVariant;
    fn rsvg_state_get_font_weight    (state: *const RsvgState) -> pango_sys::PangoWeight;
    fn rsvg_state_get_font_stretch   (state: *const RsvgState) -> pango_sys::PangoStretch;
    fn rsvg_state_get_letter_spacing (state: *const RsvgState) -> RsvgLength;
}

pub fn get_language(state: *const RsvgState) -> Option<String> {
    unsafe {
        from_glib_none(rsvg_state_get_language(state))
    }
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
