use libc;
use std::ptr;
use std::rc::Rc;

use glib::translate::*;
use glib_sys;

use handle::{self, RsvgHandle};
use load::{rsvg_load_new_node, rsvg_load_set_node_atts};
use node::{box_node, Node, RsvgNode};
use property_bag::PropertyBag;
use tree::{RsvgTree, Tree};
use util::utf8_cstr;

// A *const RsvgXmlState is just the type that we export to C
pub enum RsvgXmlState {}

struct XmlState {
    tree: Option<Box<Tree>>,
    current_node: Option<Rc<Node>>,

    // Stack of element names while parsing; used to know when to stop
    // parsing the current element.
    element_name_stack: Vec<String>,
}

impl XmlState {
    fn new() -> XmlState {
        XmlState {
            tree: None,
            current_node: None,
            element_name_stack: Vec::new(),
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

    pub fn push_element_name(&mut self, name: &str) {
        self.element_name_stack.push(name.to_string());
    }

    pub fn pop_element_name(&mut self) {
        self.element_name_stack.pop();
    }

    pub fn topmost_element_name_is(&mut self, name: &str) -> bool {
        let len = self.element_name_stack.len();

        if len > 0 {
            self.element_name_stack[len - 1] == name
        } else {
            false
        }
    }

    pub fn free_element_name_stack(&mut self) {
        self.element_name_stack.clear();
    }

    pub fn standard_element_start(
        &mut self,
        handle: *const RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) {
        let mut defs = handle::get_defs(handle);
        let mut is_svg = false;

        let new_node = rsvg_load_new_node(
            name,
            self.current_node.as_ref(),
            pbag,
            &mut defs,
            &mut is_svg,
        );

        self.push_element_name(name);

        if let Some(ref current_node) = self.current_node {
            current_node.add_child(&new_node);
        } else if is_svg {
            self.set_root(&new_node);
        }

        self.set_current_node(Some(new_node.clone()));

        rsvg_load_set_node_atts(handle, &new_node, name, pbag);
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

#[no_mangle]
pub extern "C" fn rsvg_xml_state_push_element_name(
    xml: *mut RsvgXmlState,
    name: *const libc::c_char,
) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!name.is_null());

    let name = unsafe { utf8_cstr(name) };
    xml.push_element_name(name);
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_pop_element_name(xml: *mut RsvgXmlState) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    xml.pop_element_name();
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_topmost_element_name_is(
    xml: *mut RsvgXmlState,
    name: *const libc::c_char,
) -> glib_sys::gboolean {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!name.is_null());

    let name = unsafe { utf8_cstr(name) };
    xml.topmost_element_name_is(name).to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_free_element_name_stack(xml: *mut RsvgXmlState) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    xml.free_element_name_stack();
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_standard_element_start(
    xml: *mut RsvgXmlState,
    handle: *const RsvgHandle,
    name: *const libc::c_char,
    pbag: *const PropertyBag,
) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!name.is_null());
    let name = unsafe { utf8_cstr(name) };

    assert!(!pbag.is_null());
    let pbag = unsafe { &*pbag };

    xml.standard_element_start(handle, name, pbag);
}
