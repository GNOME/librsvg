use libc;
use std;
use std::ptr;
use std::rc::Rc;
use std::str;

use handle::{self, RsvgHandle};
use load::rsvg_load_new_node;
use node::{node_new, Node, NodeType};
use property_bag::PropertyBag;
use structure::NodeSvg;
use text::NodeChars;
use tree::{RsvgTree, Tree};
use util::utf8_cstr;

/// A trait for processing a certain kind of XML subtree
///
/// In the "normal" state of processing, an `XmlContext` may create an RsvgNode
/// for each SVG element it finds, and create NodeChars inside those nodes when it
/// encounters character data.
///
/// There may be other, special contexts for different subtrees, for example,
/// for the `<style>` element.
trait XmlContext {
    /// Called when the XML parser sees the beginning of an element
    fn start_element(&mut self, handle: *mut RsvgHandle, name: &str, pbag: &PropertyBag);

    /// Called when the XML parser sees the end of an element
    fn end_element(&mut self, handle: *mut RsvgHandle, name: &str);

    /// Called when the XML parser sees character data or CDATA
    fn characters(&mut self, text: &str);

    /// Called when the context terminates, i.e. when the parent element
    /// that created this context is closed.
    fn finish(&mut self);
}

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

    /// Starts a node for an SVG element of type `name` and hooks it to the tree.
    ///
    /// `pbag` is the set of key/value pairs from the element's XML attributes.
    pub fn standard_element_start(
        &mut self,
        handle: *mut RsvgHandle,
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

        new_node.set_atts(&new_node, handle, pbag);

        // The "svg" node is special; it will parse its style attributes
        // until the end, in standard_element_end().
        if new_node.get_type() != NodeType::Svg {
            new_node.parse_style_attributes(handle, name, pbag);
        }

        new_node.set_overridden_properties();
    }

    /// Ends an SVG element for which we create a node.
    pub fn standard_element_end(&mut self, handle: *mut RsvgHandle, name: &str) {
        if let Some(ref current_node) = self.current_node.clone() {
            // The "svg" node is special; it parses its style attributes
            // here, not during element creation.
            if current_node.get_type() == NodeType::Svg {
                current_node.with_impl(|svg: &NodeSvg| {
                    svg.parse_style_attributes(current_node, handle);
                });
            }

            if self.topmost_element_name_is(name) {
                let parent = current_node.get_parent();

                self.set_current_node(parent);

                self.pop_element_name();
            }
        }
    }

    pub fn add_characters(&mut self, text: &str) {
        if text.len() == 0 {
            return;
        }

        if let Some(ref current_node) = self.current_node {
            if current_node.accept_chars() {
                let chars_node = if let Some(child) = current_node.find_last_chars_child() {
                    child
                } else {
                    let child = node_new(
                        NodeType::Chars,
                        self.current_node.as_ref(),
                        None,
                        None,
                        Box::new(NodeChars::new()),
                    );
                    current_node.add_child(&child);
                    child
                };

                chars_node.with_impl(|chars: &NodeChars| {
                    chars.append(text);
                });
            }
        }
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
pub extern "C" fn rsvg_xml_state_free_element_name_stack(xml: *mut RsvgXmlState) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    xml.free_element_name_stack();
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_standard_element_start(
    xml: *mut RsvgXmlState,
    handle: *mut RsvgHandle,
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

#[no_mangle]
pub extern "C" fn rsvg_xml_state_standard_element_end(
    xml: *mut RsvgXmlState,
    handle: *mut RsvgHandle,
    name: *const libc::c_char,
) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!name.is_null());
    let name = unsafe { utf8_cstr(name) };

    xml.standard_element_end(handle, name);
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_add_characters(
    xml: *mut RsvgXmlState,
    unterminated_text: *const libc::c_char,
    len: usize,
) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!unterminated_text.is_null());

    // libxml2 already validated the incoming string as UTF-8.  Note that
    // it is *not* nul-terminated; this is why we create a byte slice first.
    let bytes = unsafe { std::slice::from_raw_parts(unterminated_text as *const u8, len) };
    let utf8 = unsafe { str::from_utf8_unchecked(bytes) };

    xml.add_characters(utf8);
}
