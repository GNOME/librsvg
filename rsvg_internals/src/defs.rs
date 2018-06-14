use glib::translate::*;
use libc;

use node::RsvgNode;

pub enum RsvgDefs {}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_defs_register_node_by_id(
        defs: *mut RsvgDefs,
        id: *const libc::c_char,
        node: *const RsvgNode,
    );
    fn rsvg_defs_lookup(
        defs: *const RsvgDefs,
        name: *const libc::c_char,
    ) -> *mut RsvgNode;
}

pub fn register_node_by_id(defs: *mut RsvgDefs, id: &str, node: &RsvgNode) {
    unsafe {
        rsvg_defs_register_node_by_id(defs, id.to_glib_none().0, node);
    }
}

pub fn lookup(defs: *const RsvgDefs, name: &str) -> Option<&mut RsvgNode> {
    unsafe {
        let node = rsvg_defs_lookup(defs, name.to_glib_none().0);
        if node.is_null() {
            None
        } else {
            Some(&mut *node)
        }
    }
}
