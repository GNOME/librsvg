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

pub fn lookup_and_parse<T: Default + FromStr> (pbag: *const RsvgPropertyBag, key: &str) -> T {
    let value = lookup (pbag, key);

    if let Some (v) = value {
        let result = T::from_str (&v);

        if let Ok (r) = result {
            r
        } else {
            // FIXME: Error is discarded here.  Figure out a way to propagate it upstream.
            T::default ()
        }
    } else {
        T::default ()
    }
}

pub fn lookup_length (pbag: *const RsvgPropertyBag, key: &str, length_dir: LengthDir) -> RsvgLength {
    let value = lookup (pbag, key);

    if let Some (v) = value {

        // FIXME: Error is discarded here.  Figure out a way to propagate it upstream.
        RsvgLength::parse (&v, length_dir).unwrap_or (RsvgLength::default ())
    } else {
        RsvgLength::default ()
    }
}

pub fn parse_or_default<T> (pbag: *const RsvgPropertyBag, key: &'static str) -> Result <T, NodeError>
    where T: Default + FromStr<Err = ParseError>
{
    let value = lookup (pbag, key);

    if let Some (v) = value {
        T::from_str (&v).map_err (|e| NodeError::parse_error (key, e))
    } else {
        Ok (T::default ())
    }
}
