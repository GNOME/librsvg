use libc;

use std::ffi::{CStr, CString};
use std::ops::Deref;
use std::slice;
use std::str::{self, FromStr};

use crate::attributes::Attribute;

pub struct PropertyBag<'a>(Vec<(Attribute, &'a CStr)>);

pub struct OwnedPropertyBag(Vec<(Attribute, CString)>);

pub struct PropertyBagIter<'a>(slice::Iter<'a, (Attribute, &'a CStr)>);

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
    /// Creates an iterable `PropertyBag` from a C array of borrowed C strings.
    ///
    /// With libxml2's SAX parser, the caller's callback for "element start"
    /// gets passed a `xmlChar **` of attribute/value pairs.  Even indices
    /// in the array are pointers to attribute names; odd indices are
    /// pointers to attribute values.  The array terminates with a NULL
    /// element in an even index.
    ///
    /// This function is unsafe because the caller must guarantee the following:
    ///
    /// * `pairs` is a valid pointer, or NULL for an empty array
    ///
    /// * `pairs` has key/value pairs and is NULL terminated
    ///
    /// * Both keys and values are valid UTF-8, nul-terminated C strings
    ///
    /// The lifetime of the `PropertyBag` should be considered the same as the lifetime of the
    /// `pairs` array, as the property bag does not copy the strings - it directly stores pointers
    /// into that array's strings.
    pub unsafe fn new_from_key_value_pairs(pairs: *const *const libc::c_char) -> PropertyBag<'a> {
        let mut array = Vec::new();

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
                        array.push((attr, val_str));
                    }
                } else {
                    break;
                }

                i += 2;
            }
        }

        PropertyBag(array)
    }

    pub fn from_owned(owned: &OwnedPropertyBag) -> PropertyBag<'_> {
        let mut array = Vec::new();

        for &(a, ref v) in &owned.0 {
            array.push((a, v.deref()));
        }

        PropertyBag(array)
    }

    pub fn to_owned(&self) -> OwnedPropertyBag {
        let mut array = Vec::<(Attribute, CString)>::new();

        for &(a, v) in &self.0 {
            array.push((a, (*v).to_owned()));
        }

        OwnedPropertyBag(array)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> PropertyBagIter<'_> {
        PropertyBagIter(self.0.iter())
    }
}

impl<'a> Iterator for PropertyBagIter<'a> {
    type Item = (Attribute, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|&(a, v)| (a, v.to_str_utf8()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

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

        for (a, v) in pbag.iter() {
            match a {
                Attribute::Rx => {
                    assert!(v == "1");
                    had_rx = true;
                }
                Attribute::Ry => {
                    assert!(v == "2");
                    had_ry = true;
                }
                _ => unreachable!(),
            }
        }

        assert!(had_rx);
        assert!(had_ry);
    }
}
