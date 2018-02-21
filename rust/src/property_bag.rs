use libc;

use glib_sys;
use glib::translate::*;

use std::collections::HashMap;
use std::collections::hash_map;
use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::ptr;

pub struct PropertyBag<'a>(HashMap<&'a CStr, &'a CStr>);

pub struct OwnedPropertyBag(HashMap<CString, CString>);

pub struct PropertyBagIter<'a>(PropertyBagCStrIter<'a>);

pub struct PropertyBagCStrIter<'a>(hash_map::Iter<'a, &'a CStr, &'a CStr>);

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

    pub fn iter(&self) -> PropertyBagIter {
        PropertyBagIter(self.cstr_iter())
    }

    pub fn cstr_iter(&self) -> PropertyBagCStrIter {
        PropertyBagCStrIter(self.0.iter())
    }
}

impl<'a> Iterator for PropertyBagIter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k.to_str().unwrap(), v.to_str().unwrap()))
    }
}

impl<'a> Iterator for PropertyBagCStrIter<'a> {
    type Item = (&'a CStr, &'a CStr);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (*k, *v))
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

#[no_mangle]
pub extern fn rsvg_property_bag_iter_begin(pbag: *const PropertyBag) -> *mut PropertyBagCStrIter {
    assert!(!pbag.is_null());
    let pbag = unsafe { &*pbag };

    Box::into_raw(Box::new(pbag.cstr_iter()))
}

#[no_mangle]
pub extern fn rsvg_property_bag_iter_next(iter: *mut PropertyBagCStrIter,
                                          out_key: *mut *const libc::c_char,
                                          out_value: *mut *const libc::c_char)
                                          -> glib_sys::gboolean
{
    assert!(!iter.is_null());
    let iter = unsafe { &mut *iter };

    if let Some((key, val)) = iter.next() {
        unsafe {
            *out_key = key.as_ptr();
            *out_value = val.as_ptr();
        }
        true.to_glib()
    } else {
        unsafe {
            *out_key = ptr::null();
            *out_value = ptr::null();
        }
        false.to_glib()
    }
}

#[no_mangle]
pub extern fn rsvg_property_bag_iter_end(iter: *mut PropertyBagCStrIter) {
    assert!(!iter.is_null());

    unsafe { Box::from_raw(iter) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::mem;

    #[test]
    fn empty_property_bag() {
        let map = unsafe { PropertyBag::new_from_key_value_pairs(ptr::null()) };
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn property_bag_lookups_and_iters() {
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

        let pbag = unsafe { PropertyBag::new_from_key_value_pairs(v.as_ptr()) };

        assert_eq!(pbag.lookup("alpha"), Some("1"));
        assert_eq!(pbag.lookup("beta"), Some("2"));
        assert_eq!(pbag.lookup("gamma"), None);

        let mut had_alpha: bool = false;
        let mut had_beta: bool = false;

        for (k, v) in pbag.iter() {
            if k == "alpha" {
                assert!(v == "1");
                had_alpha = true;
            } else if k == "beta" {
                assert!(v == "2");
                had_beta = true;
            }
        }

        assert!(had_alpha);
        assert!(had_beta);
    }

    #[test]
    fn property_bag_can_iterate_from_c() {
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

        let pbag = unsafe { PropertyBag::new_from_key_value_pairs(v.as_ptr()) };

        let mut had_alpha: bool = false;
        let mut had_beta: bool = false;

        let iter = rsvg_property_bag_iter_begin(&pbag as *const PropertyBag);

        let mut key = unsafe { mem::uninitialized() };
        let mut val = unsafe { mem::uninitialized() };

        while from_glib(rsvg_property_bag_iter_next(iter,
                                                    &mut key as *mut _,
                                                    &mut val as *mut _)) {
            let k = unsafe { CStr::from_ptr(key).to_str().unwrap() };
            let v = unsafe { CStr::from_ptr(val).to_str().unwrap() };

            if k == "alpha" {
                assert!(v == "1");
                had_alpha = true;
            } else if k == "beta" {
                assert!(v == "2");
                had_beta = true;
            }
        }

        rsvg_property_bag_iter_end(iter);

        assert!(had_alpha);
        assert!(had_beta);
    }
}
