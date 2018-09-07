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
/// In the "normal" state of processing, an `XmlHandler` may create an RsvgNode
/// for each SVG element it finds, and create NodeChars inside those nodes when it
/// encounters character data.
///
/// There may be other, special contexts for different subtrees, for example,
/// for the `<style>` element.
trait XmlHandler {
    /// Called when the XML parser sees the beginning of an element
    fn start_element(
        &self,
        parent: Option<&Rc<Node>>,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Box<XmlHandler>;

    /// Called when the XML parser sees the end of an element.
    fn end_element(&self, handle: *mut RsvgHandle, name: &str) -> Rc<Node>;

    /// Called when the XML parser sees character data or CDATA
    fn characters(&self, text: &str);

    fn get_node(&self) -> Rc<Node>;
}

struct NodeCreationContext {
    node: Option<Rc<Node>>,
}

impl XmlHandler for NodeCreationContext {
    fn start_element(
        &self,
        parent: Option<&Rc<Node>>,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Box<XmlHandler> {
        let node = self.create_node(parent, handle, name, pbag);

        Box::new(NodeCreationContext { node: Some(node) })
    }

    fn end_element(&self, handle: *mut RsvgHandle, _name: &str) -> Rc<Node> {
        let node = self.node.as_ref().unwrap().clone();

        // The "svg" node is special; it parses its style attributes
        // here, not during element creation.
        if node.get_type() == NodeType::Svg {
            node.with_impl(|svg: &NodeSvg| {
                svg.parse_style_attributes(&node, handle);
            });
        }

        node
    }

    fn characters(&self, text: &str) {
        let node = self.node.as_ref().unwrap();

        if text.len() == 0 {
            return;
        }

        if node.accept_chars() {
            let chars_node = if let Some(child) = node.find_last_chars_child() {
                child
            } else {
                let child = node_new(
                    NodeType::Chars,
                    Some(&node),
                    None,
                    None,
                    Box::new(NodeChars::new()),
                );
                node.add_child(&child);
                child
            };

            chars_node.with_impl(|chars: &NodeChars| {
                chars.append(text);
            });
        }
    }

    fn get_node(&self) -> Rc<Node> {
        self.node.as_ref().unwrap().clone()
    }
}

impl NodeCreationContext {
    fn create_node(
        &self,
        parent: Option<&Rc<Node>>,
        handle: *const RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Rc<Node> {
        let mut defs = handle::get_defs(handle);

        let new_node = rsvg_load_new_node(name, parent, pbag, &mut defs);

        if let Some(parent) = parent {
            parent.add_child(&new_node);
        }

        new_node.set_atts(&new_node, handle, pbag);

        // The "svg" node is special; it will parse its style attributes
        // until the end, in standard_element_end().
        if new_node.get_type() != NodeType::Svg {
            new_node.parse_style_attributes(handle, name, pbag);
        }

        new_node.set_overridden_properties();

        new_node
    }
}

/// A concrete parsing context for a surrounding `element_name` and its XML event handlers
struct Context {
    element_name: String,
    handler: Box<XmlHandler>,
}

// A *const RsvgXmlState is just the type that we export to C
pub enum RsvgXmlState {}

/// Holds the state used for XML processing
///
/// These methods are called when an XML event is parsed out of the XML stream: `start_element`,
/// `end_element`, `characters`.
///
/// When an element starts, we push a corresponding `Context` into the `context_stack`.  Within
/// that context, all XML events will be forwarded to it, and processed in one of the `XmlHandler`
/// trait objects. Normally the context refers to a `NodeCreationContext` implementation which is
/// what creates normal graphical elements.
///
/// When we get to a `<style>` element, we push a `StyleContext`, which processes its contents
/// specially.
struct XmlState {
    tree: Option<Box<Tree>>,

    context_stack: Vec<Context>,
}

impl XmlState {
    fn new() -> XmlState {
        XmlState {
            tree: None,
            context_stack: Vec::new(),
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

    pub fn start_element(&mut self, handle: *mut RsvgHandle, name: &str, pbag: &PropertyBag) {
        let next_context = if let Some(top) = self.context_stack.last() {
            top.handler
                .start_element(Some(&top.handler.get_node()), handle, name, pbag)
        } else {
            let default_context = NodeCreationContext { node: None };

            default_context.start_element(None, handle, name, pbag)
        };

        let context = Context {
            element_name: name.to_string(),
            handler: next_context,
        };

        self.context_stack.push(context);
    }

    pub fn end_element(&mut self, handle: *mut RsvgHandle, name: &str) {
        if let Some(top) = self.context_stack.pop() {
            assert!(name == top.element_name);

            let node = top.handler.end_element(handle, name);

            if self.context_stack.is_empty() {
                self.set_root(&node);
            }
        } else {
            panic!("end_element: XML handler stack is empty!?");
        }
    }

    pub fn characters(&mut self, text: &str) {
        if let Some(top) = self.context_stack.last() {
            top.handler.characters(text);
        } else {
            panic!("characters: XML handler stack is empty!?");
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
pub extern "C" fn rsvg_xml_state_start_element(
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

    xml.start_element(handle, name, pbag);
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_end_element(
    xml: *mut RsvgXmlState,
    handle: *mut RsvgHandle,
    name: *const libc::c_char,
) {
    assert!(!xml.is_null());
    let xml = unsafe { &mut *(xml as *mut XmlState) };

    assert!(!name.is_null());
    let name = unsafe { utf8_cstr(name) };

    xml.end_element(handle, name);
}

#[no_mangle]
pub extern "C" fn rsvg_xml_state_characters(
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

    xml.characters(utf8);
}
