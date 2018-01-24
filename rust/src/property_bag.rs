use glib::translate::*;
use libc;
use std::ffi::CStr;

use error::*;
use parsers::Parse;

pub type FfiRsvgPropertyBag = *mut libc::c_void;

pub enum PropertyBag {
    Borrowed(FfiRsvgPropertyBag),
    Owned(FfiRsvgPropertyBag)
}

extern "C" {
    fn rsvg_property_bag_lookup (pbag: FfiRsvgPropertyBag, key: *const libc::c_char) -> *const libc::c_char;
    fn rsvg_property_bag_dup (pbag: FfiRsvgPropertyBag) -> FfiRsvgPropertyBag;
    fn rsvg_property_bag_free (pbag: FfiRsvgPropertyBag);
}

impl PropertyBag {
    pub fn new(ffi: FfiRsvgPropertyBag) -> PropertyBag {
        PropertyBag::Borrowed(ffi)
    }

    pub fn ffi(&self) -> FfiRsvgPropertyBag {
        match self {
            &PropertyBag::Borrowed(ffi) => ffi,
            &PropertyBag::Owned(ffi) => ffi
        }
    }

    pub fn dup(&self) -> PropertyBag {
        unsafe {
            PropertyBag::Owned(rsvg_property_bag_dup(self.ffi()))
        }
    }

    pub fn lookup(&self, key: &str) -> Option<&str> {
        let ffi = self.ffi();

        unsafe {
            let c_value = rsvg_property_bag_lookup (ffi, key.to_glib_none ().0);
            if c_value.is_null() {
                None
            } else {
                // we can unwrap because libxml2 already validated this for UTF-8
                Some(CStr::from_ptr(c_value).to_str().unwrap())
            }
        }
    }
}

impl Drop for PropertyBag {
    fn drop(&mut self) {
        match *self {
            PropertyBag::Borrowed(_) => (),
            PropertyBag::Owned(ffi) => unsafe { rsvg_property_bag_free(ffi) }
        }
    }
}

pub fn parse_or_none<T> (pbag: &PropertyBag,
                         key: &str,
                         data: <T as Parse>::Data,
                         validate: Option<fn(T) -> Result<T, AttributeError>>) -> Result <Option<T>, NodeError>
    where T: Parse<Err = AttributeError> + Copy
{
    let value = pbag.lookup(key);

    match value {
        Some (v) => {
            T::parse (&v, data)
                .and_then (|v|
                           if let Some(validate) = validate {
                               validate(v)
                                   .map(Some)
                           } else {
                               Ok(Some(v))
                           })
                .map_err (|e| NodeError::attribute_error (key, e))
        },

        None => Ok(None)
    }
}

pub fn parse_or_default<T> (pbag: &PropertyBag,
                            key: &str,
                            data: <T as Parse>::Data,
                            validate: Option<fn(T) -> Result<T, AttributeError>>) -> Result <T, NodeError>
    where T: Default + Parse<Err = AttributeError> + Copy
{
    parse_or_value (pbag, key, data, T::default (), validate)
}

pub fn parse_or_value<T> (pbag: &PropertyBag,
                          key: &str,
                          data: <T as Parse>::Data,
                          value: T,
                          validate: Option<fn(T) -> Result<T, AttributeError>>) -> Result <T, NodeError>
    where T: Parse<Err = AttributeError> + Copy
{
    Ok (parse_or_none (pbag, key, data, validate)?.unwrap_or (value))
}
