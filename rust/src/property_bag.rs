extern crate libc;
extern crate glib;

use self::glib::translate::*;

pub enum RsvgPropertyBag {}

extern "C" {
    fn rsvg_property_bag_size (pbag: *const RsvgPropertyBag) -> libc::c_uint;
    fn rsvg_property_bag_lookup (pbag: *const RsvgPropertyBag, key: *const libc::c_char) -> *const libc::c_char;
}

pub fn get_size (pbag: *const RsvgPropertyBag) -> usize {
    unsafe { rsvg_property_bag_size (pbag) as usize }
}

pub fn lookup (pbag: *const RsvgPropertyBag, key: &str) -> Option<String> {
    unsafe {
        let c_value = rsvg_property_bag_lookup (pbag, key.to_glib_none ().0);
        from_glib_none (c_value)
    }
}
