use glib::translate::*;
use libc;

use node::RsvgNode;

pub enum RsvgDefs {}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_defs_lookup(defs: *const RsvgDefs, url: *const libc::c_char) -> *mut RsvgNode;
}

pub fn lookup(defs: *const RsvgDefs, url: &str) -> Option<&mut RsvgNode> {
    unsafe {
        let node = rsvg_defs_lookup(defs, str::to_glib_none(url).0);

        if node.is_null() {
            None
        } else {
            Some(&mut *node)
        }
    }
}
