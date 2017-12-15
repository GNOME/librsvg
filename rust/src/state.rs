use libc;
use glib::translate::*;
use pango;
use pango_sys;

pub enum RsvgState {}

// Keep in sync with rsvg-styles.h:UnicodeBidi
#[repr(C)]
pub enum UnicodeBidi {
    Normal,
    Embed,
    Override,
}

extern "C" {
    fn rsvg_state_get_language     (state: *const RsvgState) -> *const libc::c_char;
    fn rsvg_state_get_unicode_bidi (state: *const RsvgState) -> UnicodeBidi;
    fn rsvg_state_get_text_dir     (state: *const RsvgState) -> pango_sys::PangoDirection;
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
