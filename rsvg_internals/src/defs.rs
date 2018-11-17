use libc;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ptr;
use std::rc::Rc;

use handle::{self, RsvgHandle};
use node::{Node, RsvgNode};
use util::utf8_cstr;

pub enum RsvgDefs {}

pub struct Defs {
    handle: *const RsvgHandle,
    nodes: HashMap<String, Rc<Node>>,
    externs: HashMap<String, *const RsvgHandle>,
}

impl Defs {
    fn new(handle: *const RsvgHandle) -> Defs {
        Defs {
            handle,
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
    pub fn lookup(&mut self, name: &str) -> Option<&Rc<Node>> {
        if let Ok(reference) = Reference::parse(name) {
            match reference {
                Reference::PlainUri(_) => None,
                Reference::FragmentId(fragment) => self.nodes.get(fragment),
                Reference::UriWithFragmentId(uri, fragment) => {
                    let handle = self.get_extern_handle(uri);
                    if handle.is_null() {
                        None
                    } else {
                        handle::get_defs(handle).nodes.get(fragment)
                    }
                }
            }
        } else {
            None
        }
    }

    fn get_extern_handle(&mut self, possibly_relative_uri: &str) -> *const RsvgHandle {
        handle::resolve_uri(self.handle, possibly_relative_uri).map_or(
            ptr::null(),
            |uri| match self.externs.entry(uri) {
                Entry::Occupied(e) => *(e.get()),
                Entry::Vacant(e) => {
                    let handle = handle::load_extern(self.handle, e.key());
                    if !handle.is_null() {
                        e.insert(handle);
                    }
                    handle
                }
            },
        )
    }
}

/// Represents a possibly non-canonical URI with an optional fragment identifier
///
/// Sometimes in SVG element references (e.g. the `href` in the `<feImage>` element) we
/// must decide between referencing an external file, or using a plain fragment identifier
/// like `href="#foo"` as a reference to an SVG element in the same file as the one being
/// processes.  This enum makes that distinction.
#[derive(Debug, PartialEq)]
pub enum Reference<'a> {
    PlainUri(&'a str),
    FragmentId(&'a str),
    UriWithFragmentId(&'a str, &'a str),
}

impl<'a> Reference<'a> {
    pub fn parse(s: &str) -> Result<Reference, ()> {
        let (uri, fragment) = match s.rfind('#') {
            None => (Some(s), None),
            Some(p) if p == 0 => (None, Some(&s[1..])),
            Some(p) => (Some(&s[..p]), Some(&s[(p + 1)..])),
        };

        match (uri, fragment) {
            (None, Some(f)) if f.len() == 0 => Err(()),
            (None, Some(f)) => Ok(Reference::FragmentId(f)),
            (Some(u), _) if u.len() == 0 => Err(()),
            (Some(u), None) => Ok(Reference::PlainUri(u)),
            (Some(_u), Some(f)) if f.len() == 0 => Err(()),
            (Some(u), Some(f)) => Ok(Reference::UriWithFragmentId(u, f)),
            (_, _) => Err(()),
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_defs_new(handle: *const RsvgHandle) -> *mut RsvgDefs {
    Box::into_raw(Box::new(Defs::new(handle))) as *mut RsvgDefs
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
    name: *const libc::c_char,
) -> *const RsvgNode {
    assert!(!defs.is_null());
    assert!(!name.is_null());

    let defs = unsafe { &mut *(defs as *mut Defs) };
    let name = unsafe { utf8_cstr(name) };

    match defs.lookup(name) {
        Some(n) => n as *const RsvgNode,
        None => ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_kinds() {
        assert_eq!(Reference::parse("uri"), Ok(Reference::PlainUri("uri")));
        assert_eq!(
            Reference::parse("#fragment"),
            Ok(Reference::FragmentId("fragment"))
        );
        assert_eq!(
            Reference::parse("uri#fragment"),
            Ok(Reference::UriWithFragmentId("uri", "fragment"))
        );
    }

    #[test]
    fn reference_errors() {
        assert!(Reference::parse("").is_err());
        assert!(Reference::parse("#").is_err());
        assert!(Reference::parse("uri#").is_err());
    }
}
