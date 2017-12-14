use libc;
use glib::translate::*;

pub enum RsvgState {}

extern "C" {
    fn rsvg_state_get_language(state: *const RsvgState) -> *const libc::c_char;
}

pub fn get_language(state: *const RsvgState) -> Option<String> {
    unsafe {
        from_glib_none(rsvg_state_get_language(state))
    }
}
