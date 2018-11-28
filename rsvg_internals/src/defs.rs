use libc;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ptr;
use std::rc::Rc;

use allowed_url::AllowedUrl;
use handle::{self, RsvgHandle};
use node::{Node, RsvgNode};
use util::utf8_cstr;

pub enum RsvgDefs {}

pub struct Defs {
    nodes: HashMap<String, Rc<Node>>,
    externs: HashMap<String, *const RsvgHandle>,
}

impl Defs {
    pub fn new() -> Defs {
        Defs {
            nodes: Default::default(),
            externs: Default::default(),
        }
    }

    pub fn insert(&mut self, id: &str, node: &Rc<Node>) {
        self.nodes.entry(id.to_string()).or_insert(node.clone());
    }

    /// Returns a node from an URI reference, or `None`
    ///
    /// This may return a node within the same RSVG handle, or a node in a secondary RSVG
    /// handle that is referenced by the current one.  If the element's id is not found,
    /// returns `None`.
    pub fn lookup(&mut self, handle: *const RsvgHandle, name: &str) -> Option<&Rc<Node>> {
        if let Ok(reference) = Reference::parse(name) {
            match reference {
                Reference::PlainUri(_) => None,
                Reference::FragmentId(fragment) => self.nodes.get(&fragment),
                Reference::UriWithFragmentId(href, fragment) => {
                    match self.get_extern_handle(handle, &href) {
                        Ok(extern_handle) => handle::get_defs(extern_handle).nodes.get(&fragment),
                        Err(()) => None,
                    }
                }
            }
        } else {
            None
        }
    }

    fn get_extern_handle(
        &mut self,
        handle: *const RsvgHandle,
        href: &str,
    ) -> Result<*const RsvgHandle, ()> {
        let aurl =
            AllowedUrl::from_href(href, handle::get_base_url(handle).as_ref()).map_err(|_| ())?;

        match self.externs.entry(aurl.url().as_str().to_string()) {
            Entry::Occupied(e) => Ok(*(e.get())),
            Entry::Vacant(e) => {
                let extern_handle = handle::load_extern(handle, e.key())?;
                e.insert(extern_handle);
                Ok(extern_handle)
            }
        }
    }
}

/// Represents a possibly non-canonical URI with an optional fragment identifier
///
/// Sometimes in SVG element references (e.g. the `href` in the `<feImage>` element) we
/// must decide between referencing an external file, or using a plain fragment identifier
/// like `href="#foo"` as a reference to an SVG element in the same file as the one being
/// processes.  This enum makes that distinction.
#[derive(Debug, PartialEq)]
pub enum Reference {
    PlainUri(String),
    FragmentId(String),
    UriWithFragmentId(String, String),
}

impl Reference {
    pub fn parse(s: &str) -> Result<Reference, ()> {
        let (uri, fragment) = match s.rfind('#') {
            None => (Some(s), None),
            Some(p) if p == 0 => (None, Some(&s[1..])),
            Some(p) => (Some(&s[..p]), Some(&s[(p + 1)..])),
        };

        match (uri, fragment) {
            (None, Some(f)) if f.len() == 0 => Err(()),
            (None, Some(f)) => Ok(Reference::FragmentId(f.to_string())),
            (Some(u), _) if u.len() == 0 => Err(()),
            (Some(u), None) => Ok(Reference::PlainUri(u.to_string())),
            (Some(_u), Some(f)) if f.len() == 0 => Err(()),
            (Some(u), Some(f)) => Ok(Reference::UriWithFragmentId(u.to_string(), f.to_string())),
            (_, _) => Err(()),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_defs_free(defs: *mut RsvgDefs) {
    assert!(!defs.is_null());

    unsafe {
        let defs = { &mut *(defs as *mut Defs) };
        Box::from_raw(defs);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_defs_lookup(
    defs: *mut RsvgDefs,
    handle: *const RsvgHandle,
    name: *const libc::c_char,
) -> *const RsvgNode {
    assert!(!defs.is_null());
    assert!(!name.is_null());

    let defs = unsafe { &mut *(defs as *mut Defs) };
    let name = unsafe { utf8_cstr(name) };

    match defs.lookup(handle, name) {
        Some(n) => n as *const RsvgNode,
        None => ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_kinds() {
        assert_eq!(Reference::parse("uri"), Ok(Reference::PlainUri("uri".to_string())));
        assert_eq!(
            Reference::parse("#fragment"),
            Ok(Reference::FragmentId("fragment".to_string()))
        );
        assert_eq!(
            Reference::parse("uri#fragment"),
            Ok(Reference::UriWithFragmentId("uri".to_string(), "fragment".to_string()))
        );
    }

    #[test]
    fn reference_errors() {
        assert!(Reference::parse("").is_err());
        assert!(Reference::parse("#").is_err());
        assert!(Reference::parse("uri#").is_err());
    }
}
