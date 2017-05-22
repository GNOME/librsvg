extern crate libc;
extern crate glib;

use std::str::FromStr;

use self::glib::translate::*;
use ::cairo;
use ::cairo::MatrixTrait;

use error::*;
use length::*;
use parsers::ParseError;
use transform::*;

pub enum RsvgPropertyBag {}

extern "C" {
    fn rsvg_property_bag_lookup (pbag: *const RsvgPropertyBag, key: *const libc::c_char) -> *const libc::c_char;
    fn rsvg_property_bag_dup (pbag: *const RsvgPropertyBag) -> *mut RsvgPropertyBag;
    fn rsvg_property_bag_free (pbag: *mut RsvgPropertyBag);
}

pub fn lookup (pbag: *const RsvgPropertyBag, key: &str) -> Option<String> {
    unsafe {
        let c_value = rsvg_property_bag_lookup (pbag, key.to_glib_none ().0);
        from_glib_none (c_value)
    }
}

pub fn dup (pbag: *const RsvgPropertyBag) -> *mut RsvgPropertyBag {
    unsafe {
        rsvg_property_bag_dup (pbag)
    }
}

pub fn free (pbag: *mut RsvgPropertyBag) {
    unsafe {
        rsvg_property_bag_free (pbag);
    }
}

pub fn length_or_none (pbag: *const RsvgPropertyBag, key: &'static str, length_dir: LengthDir) -> Result <Option<RsvgLength>, NodeError> {
    let value = lookup (pbag, key);

    if let Some (v) = value {
        RsvgLength::parse (&v, length_dir).map (|l| Some (l))
            .map_err (|e| NodeError::attribute_error (key, e))
    } else {
        Ok (None)
    }
}

pub fn length_or_default (pbag: *const RsvgPropertyBag, key: &'static str, length_dir: LengthDir) -> Result <RsvgLength, NodeError> {
    let r = length_or_none (pbag, key, length_dir);

    match r {
        Ok (Some (v)) => Ok (v),
        Ok (None)     => Ok (RsvgLength::default ()),
        Err (e)       => Err (e)
    }
}

pub fn length_or_value (pbag: *const RsvgPropertyBag, key: &'static str, length_dir: LengthDir, length_str: &str) -> Result <RsvgLength, NodeError> {
    let r = length_or_none (pbag, key, length_dir);

    match r {
        Ok (Some (v)) => Ok (v),
        Ok (None)     => Ok (RsvgLength::parse (length_str, length_dir).unwrap ()),
        Err (e)       => Err (e)
    }
}

pub fn parse_or_none<T> (pbag: *const RsvgPropertyBag, key: &'static str) -> Result <Option<T>, NodeError>
    where T: Default + FromStr<Err = AttributeError>
{
    let value = lookup (pbag, key);

    match value {
        Some (v) => {
            T::from_str (&v).map (|v| Some (v))
                .map_err (|e| NodeError::attribute_error (key, e))
        },

        None => Ok (None)
    }
}

pub fn parse_or_default<T> (pbag: *const RsvgPropertyBag, key: &'static str) -> Result <T, NodeError>
    where T: Default + FromStr<Err = AttributeError>
{
    let r = parse_or_none::<T> (pbag, key);

    match r {
        Ok (Some (v)) => Ok (v),
        Ok (None)     => Ok (T::default ()),
        Err (e)       => Err (e)
    }
}

pub fn parse_or_value<T> (pbag: *const RsvgPropertyBag, key: &'static str, value: T) -> Result <T, NodeError>
    where T: Default + FromStr<Err = AttributeError> + Copy
{
    let r = parse_or_none::<T> (pbag, key);

    match r {
        Ok (Some (v)) => Ok (v),
        Ok (None)     => Ok (value),
        Err (e)       => Err (e)
    }
}

pub fn transform_or_none (pbag: *const RsvgPropertyBag, key: &'static str) -> Result <Option<cairo::Matrix>, NodeError> {
    if let Some (s) = lookup (pbag, key) {
        parse_transform (&s)
            .map (|v| Some (v))
            .map_err (|e| NodeError::attribute_error (key, e))
    } else {
        Ok (None)
    }
}
