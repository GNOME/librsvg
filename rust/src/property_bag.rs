use ::cairo;
use ::glib::translate::*;
use ::libc;

use error::*;
use parsers::Parse;

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

pub fn parse_or_none<T> (pbag: *const RsvgPropertyBag,
                         key: &'static str,
                         data: <T as Parse>::Data,
                         validate: Option<fn(T) -> Result<T, AttributeError>>) -> Result <Option<T>, NodeError>
    where T: Parse<Err = AttributeError> + Copy
{
    let value = lookup (pbag, key);

    match value {
        Some (v) => {
            T::parse (&v, data)
                .and_then (|v|
                           if let Some(validate) = validate {
                               validate(v)
                                   .map(|v| Some(v))
                           } else {
                               Ok(Some(v))
                           })
                .map_err (|e| NodeError::attribute_error (key, e))
        },

        None => Ok(None)
    }
}

pub fn parse_or_default<T> (pbag: *const RsvgPropertyBag,
                            key: &'static str,
                            data: <T as Parse>::Data,
                            validate: Option<fn(T) -> Result<T, AttributeError>>) -> Result <T, NodeError>
    where T: Default + Parse<Err = AttributeError> + Copy
{
    parse_or_value (pbag, key, data, T::default (), validate)
}

pub fn parse_or_value<T> (pbag: *const RsvgPropertyBag,
                          key: &'static str,
                          data: <T as Parse>::Data,
                          value: T,
                          validate: Option<fn(T) -> Result<T, AttributeError>>) -> Result <T, NodeError>
    where T: Parse<Err = AttributeError> + Copy
{
    Ok (parse_or_none (pbag, key, data, validate)?.unwrap_or (value))
}
