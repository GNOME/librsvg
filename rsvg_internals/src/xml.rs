use std::ptr;
use std::rc::Rc;

use node::{Node, RsvgNode};
use tree::{RsvgTree, Tree};

// A *const RsvgXmlState is just the type that we export to C
pub enum RsvgXmlState {}

struct XmlState {
    tree: Option<Box<Tree>>,
}

impl XmlState {
    fn new() -> XmlState {
        XmlState { tree: None }
    }

    pub fn set_root(&mut self, root: &Rc<Node>) {
        if self.tree.is_some() {
            panic!("The tree root has already been set");
        }

        self.tree = Some(Box::new(Tree::new(root)));
    }

    pub fn steal_tree(&mut self) -> Option<Box<Tree>> {
        self.tree.take()
    }
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_new() -> *mut RsvgXmlState {
    Box::into_raw(Box::new(XmlState::new())) as *mut RsvgXmlState
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_free(xml: *mut RsvgXmlState) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };
    unsafe {
        Box::from_raw(xml);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_set_root(xml: *mut RsvgXmlState, root: *const RsvgNode) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!root.is_null());
    let root = unsafe { &*root };

    xml.set_root(root);
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_steal_tree(xml: *mut RsvgXmlState) -> *mut RsvgTree {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    if let Some(tree) = xml.steal_tree() {
        Box::into_raw(tree) as *mut RsvgTree
    } else {
        ptr::null_mut()
    }
}
