use libc;

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::ptr;

use error::*;
use parsers::Parse;

pub struct PropertyBag<'a>(HashMap<&'a CStr, &'a CStr>);

pub struct OwnedPropertyBag(HashMap<CString, CString>);

impl<'a> PropertyBag<'a> {
    pub unsafe fn new_from_key_value_pairs(pairs: *const *const libc::c_char) -> PropertyBag<'a> {
        let mut map = HashMap::new();

        if !pairs.is_null() {
            let mut i = 0;
            loop {
                let key = *pairs.offset(i);
                if !key.is_null() {
                    let val = *pairs.offset(i + 1);
                    assert!(!val.is_null());

                    let key_str = CStr::from_ptr(key);
                    let val_str = CStr::from_ptr(val);

                    map.insert(key_str, val_str);
                } else {
                    break;
                }

                i += 2;
            }
        }

        PropertyBag(map)
    }

    pub fn from_owned(owned: &OwnedPropertyBag) -> PropertyBag {
        let mut map = HashMap::new();

        for (k, v) in &owned.0 {
            map.insert(k.deref(), v.deref());
        }

        PropertyBag(map)
    }

    pub fn to_owned(&self) -> OwnedPropertyBag {
        let mut map = HashMap::<CString, CString>::new();

        for (k, v) in &self.0 {
            map.insert((*k).to_owned(), (*v).to_owned());
        }

        OwnedPropertyBag(map)
    }

    pub fn ffi(&self) -> *const PropertyBag {
        self as *const PropertyBag
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn lookup_cstr(&self, key: &CStr) -> Option<&CStr> {
        self.0.get(key).map(|v| *v)
    }

    pub fn lookup(&self, key: &str) -> Option<&str> {
        let k = CString::new(key).unwrap();
        self.lookup_cstr(&k).map(|v| v.to_str().unwrap())
    }

    pub fn enumerate(&self,
                 enum_fn: fn (key: *const libc::c_char, val: *const libc::c_char, data: *const libc::c_void),
                 data: *const libc::c_void) {
        for (k, v) in &self.0 {
            enum_fn(k.as_ptr(), v.as_ptr(), data);
        }
    }
}

#[no_mangle]
pub extern fn rsvg_property_bag_new<'a>(atts: *const *const libc::c_char) -> *const PropertyBag<'a> {
    let pbag = unsafe { PropertyBag::new_from_key_value_pairs(atts) };
    Box::into_raw(Box::new(pbag))
}

#[no_mangle]
pub extern fn rsvg_property_bag_free(pbag: *mut PropertyBag) {
    unsafe {
        let _ = Box::from_raw(pbag);
    }
}

#[no_mangle]
pub extern fn rsvg_property_bag_size(pbag: *const PropertyBag) -> libc::c_uint {
    unsafe {
        let pbag = &*pbag;

        pbag.len() as libc::c_uint
    }
}

#[no_mangle]
pub extern fn rsvg_property_bag_enumerate(pbag: *const PropertyBag,
                                          enum_fn: fn (key: *const libc::c_char, val: *const libc::c_char, data: *const libc::c_void),
                                          data: *const libc::c_void) {
    unsafe {
        let pbag = &*pbag;

        pbag.enumerate(enum_fn, data);
    }
}

#[no_mangle]
pub extern fn rsvg_property_bag_lookup(pbag: *const PropertyBag,
                                       raw_key: *const libc::c_char) -> *const libc::c_char {
    unsafe {
        let pbag = &*pbag;
        let key = CStr::from_ptr(raw_key);
        match pbag.lookup_cstr(key) {
            Some(v) => v.as_ptr(),
            None => ptr::null()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn empty_property_bag() {
        let map = unsafe { PropertyBag::new_from_key_value_pairs(ptr::null()) };
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn property_bag_lookups() {
        let pairs = [
            CString::new("alpha").unwrap(),
            CString::new("1").unwrap(),
            CString::new("beta").unwrap(),
            CString::new("2").unwrap(),
        ];

        let mut v = Vec::new();

        for x in &pairs {
            v.push(x.as_ptr() as *const libc::c_char);
        }

        v.push(ptr::null());

        let map = unsafe { PropertyBag::new_from_key_value_pairs(v.as_ptr()) };

        assert_eq!(map.lookup("alpha"), Some("1"));
        assert_eq!(map.lookup("beta"), Some("2"));
        assert_eq!(map.lookup("gamma"), None);
    }
}
