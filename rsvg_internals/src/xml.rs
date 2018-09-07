use std::ptr;
use std::rc::Rc;

use node::{box_node, Node, RsvgNode};
use tree::{RsvgTree, Tree};

// A *const RsvgXmlState is just the type that we export to C
pub enum RsvgXmlState {}

struct XmlState {
    tree: Option<Box<Tree>>,
    current_node: Option<Rc<Node>>,
}

impl XmlState {
    fn new() -> XmlState {
        XmlState {
            tree: None,
            current_node: None,
        }
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

    pub fn get_current_node(&self) -> Option<Rc<Node>> {
        self.current_node.clone()
    }

    pub fn set_current_node(&mut self, node: Option<Rc<Node>>) {
        self.current_node = node;
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

#[no_mangle]
pub extern "C" fn rsvg_xml_state_get_current_node(xml: *const RsvgXmlState) -> *mut RsvgNode {
    assert!(!xml.is_null());
    let xml = unsafe { &*(xml as *const XmlState) };

    if let Some(ref node) = xml.get_current_node() {
        box_node(node.clone())
    } else {
        ptr::null_mut()
    }
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_set_current_node(
    xml: *mut RsvgXmlState,
    raw_node: *const RsvgNode,
) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    let node = if raw_node.is_null() {
        None
    } else {
        let n = unsafe { &*raw_node };
        Some(n.clone())
    };

    xml.set_current_node(node);
}
