use libc;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ptr;
use std::rc::Rc;

use handle::{self, RsvgHandle};
use node::{Node, RsvgNode};
use util::utf8_cstr;

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

    pub fn lookup(&mut self, name: &str) -> Option<&Rc<Node>> {
        match name.rfind('#') {
            None => None,
            Some(p) if p == 0 => self.nodes.get(&name[1..]),
            Some(p) => {
                let handle = self.get_extern_handle(&name[..p]);
                if handle.is_null() {
                    None
                } else {
                    handle::get_defs(handle).nodes.get(&name[(p + 1)..])
                }
            }
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

#[no_mangle]
pub extern "C" fn rsvg_defs_new(handle: *const RsvgHandle) -> *mut Defs {
    Box::into_raw(Box::new(Defs::new(handle)))
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_defs_free(defs: *mut Defs) {
    assert!(!defs.is_null());
    Box::from_raw(defs);
}

#[no_mangle]
pub extern "C" fn rsvg_defs_lookup(defs: *mut Defs, name: *const libc::c_char) -> *const RsvgNode {
    assert!(!defs.is_null());
    assert!(!name.is_null());

    let defs = unsafe { &mut *defs };
    let name = unsafe { utf8_cstr(name) };

    match defs.lookup(name) {
        Some(n) => n as *const RsvgNode,
        None => ptr::null(),
    }
}
