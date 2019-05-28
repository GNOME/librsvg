use libc;

use std::ffi::CStr;
use std::mem;
use std::slice;
use std::str;

use markup5ever::LocalName;

enum Attribute<'a> {
    CStr(&'a CStr),
    Unterminated(&'a str),
}

pub struct PropertyBag<'a>(Vec<(LocalName, Attribute<'a>)>);

pub struct PropertyBagIter<'a>(slice::Iter<'a, (LocalName, Attribute<'a>)>);

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

                    let attr = LocalName::from(key_str.to_str_utf8());
                    array.push((attr, Attribute::CStr(val_str)));
                } else {
                    break;
                }

                i += 2;
            }
        }

        PropertyBag(array)
    }

    /// Creates an iterable `PropertyBag` from a C array of borrowed C strings.
    ///
    /// With libxml2's SAX parser, the caller's startElementNsSAX2Func
    /// callback gets passed a `xmlChar **` for attributes, which
    /// comes in groups of /// (localname/prefix/URI/value_start/value_end).
    /// In those, localname/prefix/URI are NUL-terminated strings;
    /// value_start and value_end point to the start-inclusive and
    /// end-exclusive bytes in the attribute's value.
    ///
    /// This function is unsafe because the caller must guarantee the following:
    ///
    /// * `attrs` is a valid pointer, with (n_attributes * 5) elements.
    ///
    /// * All strings are valid UTF-8.
    ///
    /// The lifetime of the `PropertyBag` should be considered the same as the lifetime of the
    /// `attrs` array, as the property bag does not copy the strings - it directly stores pointers
    /// into that array's strings.
    pub unsafe fn new_from_namespaced_attributes(
        n_attributes: usize,
        attrs: *const *const libc::c_char,
    ) -> PropertyBag<'a> {
        let mut array = Vec::new();

        if n_attributes > 0 && !attrs.is_null() {
            let attrs = slice::from_raw_parts(attrs, n_attributes * 5);

            let mut i = 0;
            while i < n_attributes * 5 {
                let localname = attrs[i];
                let _prefix = attrs[i + 1];
                let _uri = attrs[i + 2];
                let value_start = attrs[i + 3];
                let value_end = attrs[i + 4];

                assert!(!localname.is_null());

                if !value_start.is_null() && !value_end.is_null() {
                    assert!(value_end >= value_start);

                    // FIXME: ptr::offset_from() is nightly-only.
                    // We'll do the computation of the length by hand.
                    let start: usize = mem::transmute(value_start);
                    let end: usize = mem::transmute(value_end);
                    let len = end - start;

                    let value_slice = slice::from_raw_parts(value_start as *const u8, len);
                    let value_str = str::from_utf8_unchecked(value_slice);

                    let key_str = CStr::from_ptr(localname);
                    let attr = LocalName::from(key_str.to_str_utf8());
                    array.push((attr, Attribute::Unterminated(value_str)));
                }

                i += 5;
            }
        }

        PropertyBag(array)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> PropertyBagIter<'_> {
        PropertyBagIter(self.0.iter())
    }
}

impl<'a> Iterator for PropertyBagIter<'a> {
    type Item = (LocalName, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(a, v)| match *v {
            Attribute::CStr(ref v) => (a.clone(), v.to_str_utf8()),
            Attribute::Unterminated(v) => (a.clone(), v),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use markup5ever::local_name;
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
                local_name!("rx") => {
                    assert!(v == "1");
                    had_rx = true;
                }
                local_name!("ry") => {
                    assert!(v == "2");
                    had_ry = true;
                }
                _ => unreachable!(),
            }
        }

        assert!(had_rx);
        assert!(had_ry);
    }

    #[test]
    fn property_bag_with_namespaces() {
        let attrs = [
            (CString::new("rx").unwrap(), CString::new("1").unwrap()),
            (CString::new("ry").unwrap(), CString::new("2").unwrap()),
            (CString::new("empty").unwrap(), CString::new("").unwrap()),
        ];

        let mut v: Vec<*const libc::c_char> = Vec::new();

        for (key, val) in &attrs {
            v.push(key.as_ptr() as *const libc::c_char); // localname
            v.push(ptr::null()); // prefix
            v.push(ptr::null()); // uri

            let val_start = val.as_ptr() as *const libc::c_char;
            let val_end = unsafe { val_start.offset(val.as_bytes().len() as isize) };
            v.push(val_start); // value_start
            v.push(val_end); // value_end
        }

        let pbag = unsafe { PropertyBag::new_from_namespaced_attributes(2, v.as_ptr()) };

        let mut had_rx: bool = false;
        let mut had_ry: bool = false;
        let mut had_empty: bool = false;

        for (a, v) in pbag.iter() {
            match a {
                local_name!("rx") => {
                    assert!(v == "1");
                    had_rx = true;
                }
                local_name!("ry") => {
                    assert!(v == "2");
                    had_ry = true;
                }
                ref n if *n == LocalName::from("empty") => {
                    assert!(v == "");
                    had_empty = true;
                }
                _ => unreachable!(),
            }
        }

        assert!(had_rx);
        assert!(had_ry);
    }
}
