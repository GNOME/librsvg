extern crate phf;

use glib::translate::*;
use glib_sys;
use libc;
use std::str::FromStr;

use util::utf8_cstr;

include!(concat!(env!("OUT_DIR"), "/attributes-codegen.rs"));

impl FromStr for Attribute {
    type Err = ();

    fn from_str(s: &str) -> Result<Attribute, ()> {
        ATTRIBUTES.get(s).cloned().ok_or(())
    }
}

#[no_mangle]
pub extern "C" fn rsvg_attribute_from_name(raw_name: *const libc::c_char,
                                           out_attr: *mut Attribute)
                                           -> glib_sys::gboolean {
    let name = unsafe { utf8_cstr(raw_name) };

    match Attribute::from_str(name) {
        Ok(a) => {
            unsafe {
                *out_attr = a;
            }
            true.to_glib()
        }

        Err(_) => false.to_glib(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn parses_attributes() {
        assert_eq!(Attribute::from_str("width"), Ok(Attribute::Width));
    }

    #[test]
    fn unknown_attribute_yields_error() {
        assert_eq!(Attribute::from_str("foobar"), Err(()));
    }

    #[test]
    fn c_attribute_from_name() {
        let mut a: Attribute = unsafe { mem::uninitialized() };
        let res: bool =
            from_glib(rsvg_attribute_from_name("width".to_glib_none().0, &mut a as *mut Attribute));
        assert!(res);
        assert_eq!(a, Attribute::Width);
    }

    #[test]
    fn invalid_c_attribute_from_name() {
        let mut a: Attribute = unsafe { mem::uninitialized() };
        let res: bool = from_glib(rsvg_attribute_from_name("foobar".to_glib_none().0,
                                                           &mut a as *mut Attribute));
        assert!(!res);
    }
}
