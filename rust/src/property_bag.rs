extern crate libc;
extern crate glib;

use std::str::FromStr;

use self::glib::translate::*;

use error::*;
use length::*;
use parsers::ParseError;

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

pub fn length_or_default (pbag: *const RsvgPropertyBag, key: &'static str, length_dir: LengthDir) -> Result <RsvgLength, NodeError> {
    let value = lookup (pbag, key);

    if let Some (v) = value {
        RsvgLength::parse (&v, length_dir).map_err (|e| NodeError::attribute_error (key, e))
    } else {
        Ok (RsvgLength::default ())
    }
}

pub fn parse_or_default<T> (pbag: *const RsvgPropertyBag, key: &'static str) -> Result <T, NodeError>
    where T: Default + FromStr<Err = AttributeError>
{
    let value = lookup (pbag, key);

    if let Some (v) = value {
        T::from_str (&v).map_err (|e| NodeError::attribute_error (key, e))
    } else {
        Ok (T::default ())
    }
}
