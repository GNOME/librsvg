use libc;

use glib_sys;
use glib::translate::*;

use std::collections::HashMap;
use std::collections::hash_map;
use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::ptr;
use std::str::{self, FromStr};

use attributes::Attribute;

pub struct PropertyBag<'a>(HashMap<&'a CStr, (Attribute, &'a CStr)>);

pub struct OwnedPropertyBag(HashMap<CString, (Attribute, CString)>);

pub struct PropertyBagIter<'a>(PropertyBagCStrIter<'a>);

pub struct PropertyBagCStrIter<'a>(hash_map::Iter<'a, &'a CStr, (Attribute, &'a CStr)>);

trait Utf8CStrToStr {
    fn to_str_utf8(&self) -> &str;
}

impl Utf8CStrToStr for CStr {
    fn to_str_utf8(&self) -> &str {
        // We can *only* do this when the CStr comes from a C string that was validated
        // as UTF-8 on the C side of things.  In our case, the C strings from libxml2 and
        // are valid UTF-8.
        unsafe { str::from_utf8_unchecked(self.to_bytes()) }
    }
}

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

                    // We silently drop unknown attributes.  New attributes should be added in
                    // build.rs.
                    if let Ok(attr) = Attribute::from_str(key_str.to_str_utf8()) {
                        map.insert(key_str, (attr, val_str));
                    }
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

        for (k, &(a, ref v)) in &owned.0 {
            map.insert(k.deref(), (a, v.deref()));
        }

        PropertyBag(map)
    }

    pub fn to_owned(&self) -> OwnedPropertyBag {
        let mut map = HashMap::<CString, (Attribute, CString)>::new();

        for (k, &(a, v)) in &self.0 {
            map.insert((*k).to_owned(), (a, (*v).to_owned()));
        }

        OwnedPropertyBag(map)
    }

    pub fn ffi(&self) -> *const PropertyBag {
        self as *const PropertyBag
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> PropertyBagIter {
        PropertyBagIter(self.cstr_iter())
    }

    pub fn cstr_iter(&self) -> PropertyBagCStrIter {
        PropertyBagCStrIter(self.0.iter())
    }
}

impl<'a> Iterator for PropertyBagIter<'a> {
    type Item = (&'a str, Attribute, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, a, v)| (k.to_str_utf8(), a, v.to_str_utf8()))
    }
}

impl<'a> Iterator for PropertyBagCStrIter<'a> {
    type Item = (&'a CStr, Attribute, &'a CStr);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, &(a, v))| (*k, a, v))
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
pub extern fn rsvg_property_bag_iter_begin(pbag: *const PropertyBag) -> *mut PropertyBagCStrIter {
    assert!(!pbag.is_null());
    let pbag = unsafe { &*pbag };

    Box::into_raw(Box::new(pbag.cstr_iter()))
}

#[no_mangle]
pub extern fn rsvg_property_bag_iter_next(iter: *mut PropertyBagCStrIter,
                                          out_key: *mut *const libc::c_char,
                                          out_attr: *mut Attribute,
                                          out_value: *mut *const libc::c_char)
                                          -> glib_sys::gboolean
{
    assert!(!iter.is_null());
    let iter = unsafe { &mut *iter };

    if let Some((key, attr, val)) = iter.next() {
        unsafe {
            *out_key = key.as_ptr();
            *out_attr = attr;
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
    fn property_bag_iters() {
        let pairs = [
            CString::new("rx").unwrap(),
            CString::new("1").unwrap(),
            CString::new("ry").unwrap(),
            CString::new("2").unwrap(),
        ];

        let mut v = Vec::new();

        for x in &pairs {
            v.push(x.as_ptr() as *const libc::c_char);
        }

        v.push(ptr::null());

        let pbag = unsafe { PropertyBag::new_from_key_value_pairs(v.as_ptr()) };

        let mut had_rx: bool = false;
        let mut had_ry: bool = false;

        for (k, a, v) in pbag.iter() {
            if k == "rx" {
                assert!(a == Attribute::Rx);
                assert!(v == "1");
                had_rx = true;
            } else if k == "ry" {
                assert!(a == Attribute::Ry);
                assert!(v == "2");
                had_ry = true;
            } else {
                unreachable!();
            }
        }

        assert!(had_rx);
        assert!(had_ry);
    }

    #[test]
    fn property_bag_can_iterate_from_c() {
        let pairs = [
            CString::new("rx").unwrap(),
            CString::new("1").unwrap(),
            CString::new("ry").unwrap(),
            CString::new("2").unwrap(),
        ];

        let mut v = Vec::new();

        for x in &pairs {
            v.push(x.as_ptr() as *const libc::c_char);
        }

        v.push(ptr::null());

        let pbag = unsafe { PropertyBag::new_from_key_value_pairs(v.as_ptr()) };

        let mut had_rx: bool = false;
        let mut had_ry: bool = false;

        let iter = rsvg_property_bag_iter_begin(&pbag as *const PropertyBag);

        let mut key = unsafe { mem::uninitialized() };
        let mut att = unsafe { mem::uninitialized() };
        let mut val = unsafe { mem::uninitialized() };

        while from_glib(rsvg_property_bag_iter_next(iter,
                                                    &mut key as *mut _,
                                                    &mut att as *mut _,
                                                    &mut val as *mut _)) {
            let k = unsafe { CStr::from_ptr(key).to_str_utf8() };
            let v = unsafe { CStr::from_ptr(val).to_str_utf8() };

            if k == "rx" {
                assert!(att == Attribute::Rx);
                assert!(v == "1");
                had_rx = true;
            } else if k == "ry" {
                assert!(att == Attribute::Ry);
                assert!(v == "2");
                had_ry = true;
            }
        }

        rsvg_property_bag_iter_end(iter);

        assert!(had_rx);
        assert!(had_ry);
    }
}
