use glib::translate::*;
use glib_sys;

use std::cell::Cell;
use std::rc::Rc;

use node::{box_node, Node, NodeType, RsvgNode};
use state::ComputedValues;

pub struct Tree {
    pub root: Rc<Node>,
    already_cascaded: Cell<bool>,
}

impl Tree {
    pub fn new(root: &Rc<Node>) -> Tree {
        Tree {
            root: root.clone(),
            already_cascaded: Cell::new(false),
        }
    }

    pub fn cascade(&self) {
        if !self.already_cascaded.get() {
            self.already_cascaded.set(true);
            let values = ComputedValues::default();
            self.root.cascade(&values);
        }
    }

    fn root_is_svg(&self) -> bool {
        self.root.get_type() == NodeType::Svg
    }
}

#[no_mangle]
pub extern "C" fn rsvg_tree_new(root: *const RsvgNode) -> *mut Tree {
    assert!(!root.is_null());
    let root: &RsvgNode = unsafe { &*root };

    Box::into_raw(Box::new(Tree::new(root)))
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_tree_free(tree: *mut Tree) {
    if !tree.is_null() {
        Box::from_raw(tree);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_tree_cascade(tree: *const Tree) {
    assert!(!tree.is_null());
    let tree = unsafe { &*tree };

    tree.cascade();
}

#[no_mangle]
pub extern "C" fn rsvg_tree_get_root(tree: *const Tree) -> *mut RsvgNode {
    assert!(!tree.is_null());
    let tree = unsafe { &*tree };

    box_node(tree.root.clone())
}

#[no_mangle]
pub extern "C" fn rsvg_tree_is_root(tree: *const Tree, node: *mut RsvgNode) -> glib_sys::gboolean {
    assert!(!tree.is_null());
    let tree = unsafe { &*tree };

    assert!(!node.is_null());
    let node: &RsvgNode = unsafe { &*node };

    Rc::ptr_eq(&tree.root, node).to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_tree_root_is_svg(tree: *const Tree) -> glib_sys::gboolean {
    assert!(!tree.is_null());
    let tree = unsafe { &*tree };

    tree.root_is_svg().to_glib()
}
