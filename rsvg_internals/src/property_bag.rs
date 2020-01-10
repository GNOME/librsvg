//! Iterate among libxml2's arrays of XML attribute/value.

use libc;

use std::mem;
use std::slice;
use std::str;

use markup5ever::{namespace_url, LocalName, Namespace, Prefix, QualName};

use crate::util::{opt_utf8_cstr, utf8_cstr};

/// Iterable wrapper for libxml2's representation of attribute/value.
///
/// See the [`new_from_xml2_attributes`] function for information.
///
/// [`new_from_xml2_attributes`]: #method.new_from_xml2_attributes
pub struct PropertyBag<'a>(Vec<(QualName, &'a str)>);

/// Iterator from `PropertyBag.iter`.
pub struct PropertyBagIter<'a>(slice::Iter<'a, (QualName, &'a str)>);

impl<'a> PropertyBag<'a> {
    /// Creates an iterable `PropertyBag` from the C array of borrowed C strings.
    ///
    /// With libxml2's SAX parser, the caller's startElementNsSAX2Func
    /// callback gets passed a `xmlChar **` for attributes, which
    /// comes in groups of (localname/prefix/URI/value_start/value_end).
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
    pub unsafe fn new_from_xml2_attributes(
        n_attributes: usize,
        attrs: *const *const libc::c_char,
    ) -> PropertyBag<'a> {
        let mut array = Vec::new();

        if n_attributes > 0 && !attrs.is_null() {
            for attr in slice::from_raw_parts(attrs, n_attributes * 5).chunks_exact(5) {
                let localname = attr[0];
                let prefix = attr[1];
                let uri = attr[2];
                let value_start = attr[3];
                let value_end = attr[4];

                assert!(!localname.is_null());

                let localname = utf8_cstr(localname);

                let prefix = opt_utf8_cstr(prefix);
                let uri = opt_utf8_cstr(uri);
                let qual_name = QualName::new(
                    prefix.map(Prefix::from),
                    uri.map(Namespace::from)
                        .unwrap_or_else(|| namespace_url!("")),
                    LocalName::from(localname),
                );

                if !value_start.is_null() && !value_end.is_null() {
                    assert!(value_end >= value_start);

                    // FIXME: ptr::offset_from() is nightly-only.
                    // We'll do the computation of the length by hand.
                    let start: usize = mem::transmute(value_start);
                    let end: usize = mem::transmute(value_end);
                    let len = end - start;

                    let value_slice = slice::from_raw_parts(value_start as *const u8, len);
                    let value_str = str::from_utf8_unchecked(value_slice);

                    array.push((qual_name, value_str));
                }
            }
        }

        PropertyBag(array)
    }

    /// Returns the number of attributes in the property bag.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Creates an iterator that yields `(QualName, &'a str)` tuples.
    pub fn iter(&self) -> PropertyBagIter<'_> {
        PropertyBagIter(self.0.iter())
    }
}

impl<'a> Iterator for PropertyBagIter<'a> {
    type Item = (QualName, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(a, v)| (a.clone(), v.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use markup5ever::{expanded_name, local_name, namespace_url, ns};
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn empty_property_bag() {
        let map = unsafe { PropertyBag::new_from_xml2_attributes(0, ptr::null()) };
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn property_bag_with_namespaces() {
        let attrs = [
            (
                CString::new("href").unwrap(),
                Some(CString::new("xlink").unwrap()),
                Some(CString::new("http://www.w3.org/1999/xlink").unwrap()),
                CString::new("1").unwrap(),
            ),
            (
                CString::new("ry").unwrap(),
                None,
                None,
                CString::new("2").unwrap(),
            ),
            (
                CString::new("d").unwrap(),
                None,
                None,
                CString::new("").unwrap(),
            ),
        ];

        let mut v: Vec<*const libc::c_char> = Vec::new();

        for (localname, prefix, uri, val) in &attrs {
            v.push(localname.as_ptr() as *const libc::c_char);
            v.push(
                prefix
                    .as_ref()
                    .map(|p: &CString| p.as_ptr())
                    .unwrap_or_else(|| ptr::null()) as *const libc::c_char,
            );
            v.push(
                uri.as_ref()
                    .map(|p: &CString| p.as_ptr())
                    .unwrap_or_else(|| ptr::null()) as *const libc::c_char,
            );

            let val_start = val.as_ptr() as *const libc::c_char;
            let val_end = unsafe { val_start.offset(val.as_bytes().len() as isize) };
            v.push(val_start); // value_start
            v.push(val_end); // value_end
        }

        let pbag = unsafe { PropertyBag::new_from_xml2_attributes(3, v.as_ptr()) };

        let mut had_href: bool = false;
        let mut had_ry: bool = false;
        let mut had_d: bool = false;

        for (a, v) in pbag.iter() {
            match a.expanded() {
                expanded_name!(xlink "href") => {
                    assert!(v == "1");
                    had_href = true;
                }

                expanded_name!("", "ry") => {
                    assert!(v == "2");
                    had_ry = true;
                }

                expanded_name!("", "d") => {
                    assert!(v == "");
                    had_d = true;
                }

                _ => unreachable!(),
            }
        }

        assert!(had_href);
        assert!(had_ry);
        assert!(had_d);
    }
}
