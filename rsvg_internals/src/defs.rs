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
}

pub fn register_node_by_id(defs: *mut RsvgDefs, id: &str, node: &RsvgNode) {
    unsafe {
        rsvg_defs_register_node_by_id(defs, id.to_glib_none().0, node);
    }
}
