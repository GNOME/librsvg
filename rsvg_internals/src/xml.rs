use encoding::label::encoding_from_whatwg_label;
use encoding::DecoderTrap;
use libc;
use std;
use std::cell::RefCell;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::str;

use attributes::Attribute;
use css;
use handle::{self, RsvgHandle};
use load::rsvg_load_new_node;
use node::{node_new, Node, NodeType};
use property_bag::PropertyBag;
use structure::NodeSvg;
use text::NodeChars;
use tree::{RsvgTree, Tree};
use util::utf8_cstr;
// struct XIncludeContext {
// needs_fallback: bool,
// }
//
// impl XmlHandler for XIncludeContext {
// fn start_element(
// &self,
// _previous_handler: Option<&XmlHandler>,
// _parent: Option<&Rc<Node>>,
// handle: *mut RsvgHandle,
// _name: &str,
// pbag: &PropertyBag,
// ) -> Box<XmlHandler> {
// let mut href = None;
// let mut parse = None;
// let mut encoding = None;
//
// for (_key, attr, value) in pbag.iter() {
// match attr {
// Attribute::Href => href = Some(value),
// Attribute::Parse => parse = Some(value),
// Attribute::Encoding => encoding = Some(value),
// _ => (),
// }
// }
//
// self.acquire(handle, href, parse, encoding);
//
// unimplemented!("finish start_xinclude() here");
//
// Box::new(XIncludeContext::empty())
// }
//
// fn end_element(&self, handle: *mut RsvgHandle, _name: &str) -> Option<Rc<Node>> {
// unimplemented!();
// }
//
// fn characters(&self, text: &str) {
// unimplemented!();
// }
// }
//
// impl XIncludeContext {
// fn empty() -> XIncludeContext {
// XIncludeContext {
// needs_fallback: true,
// }
// }
//
// fn acquire(
// &self,
// handle: *mut RsvgHandle,
// href: Option<&str>,
// parse: Option<&str>,
// encoding: Option<&str>,
// ) {
// if let Some(href) = href {
// if parse == Some("text") {
// self.acquire_text(handle, href, encoding);
// } else {
// unimplemented!("finish the xml case here");
// }
// }
// }
//
// fn acquire_text(&self, handle: *mut RsvgHandle, href: &str, encoding: Option<&str>) {
// let binary = match handle::acquire_data(handle, href) {
// Ok(b) => b,
// Err(e) => {
// rsvg_log!("could not acquire \"{}\": {}", href, e);
// return;
// }
// };
//
// let encoding = encoding.unwrap_or("utf-8");
//
// let encoder = match encoding_from_whatwg_label(encoding) {
// Some(enc) => enc,
// None => {
// rsvg_log!("unknown encoding \"{}\" for \"{}\"", encoding, href);
// return;
// }
// };
//
// let utf8_data = match encoder.decode(&binary.data, DecoderTrap::Strict) {
// Ok(data) => data,
//
// Err(e) => {
// rsvg_log!(
// "could not convert contents of \"{}\" from character encoding \"{}\": {}",
// href,
// encoding,
// e
// );
// return;
// }
// };
//
// unimplemented!("rsvg_xml_state_characters(utf8_data)");
// }
// }
enum ContextKind {
    // Starting state
    Start,

    // Creating nodes for elements under the given parent
    ElementCreation(Rc<Node>),

    // Inside a <style> element
    Style(StyleContext),

    // An element inside a <style> context, to be ignored
    UnsupportedStyleChild,

    // Inside <xi:include>
    XInclude,
}

/// Handles the `<style>` element by parsing its character contents as CSS
struct StyleContext {
    is_text_css: bool,
    text: String,
}

/// A concrete parsing context for a surrounding `element_name` and its XML event handlers
struct Context {
    element_name: String,
    kind: ContextKind,
}

impl Context {
    fn empty() -> Context {
        Context {
            element_name: String::new(),
            kind: ContextKind::Start,
        }
    }
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
    context: Context,
    context_stack: Vec<Context>,
}

fn style_characters(style_ctx: &mut StyleContext, text: &str) {
    style_ctx.text.push_str(text);
}

impl XmlState {
    fn new() -> XmlState {
        XmlState {
            tree: None,
            context: Context::empty(),
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

    fn push_context(&mut self, ctx: Context) {
        let top = mem::replace(&mut self.context, ctx);
        self.context_stack.push(top);
    }

    pub fn start_element(&mut self, handle: *mut RsvgHandle, name: &str, pbag: &PropertyBag) {
        let new_ctx = match self.context.kind {
            ContextKind::Start => self.element_creation_start_element(None, handle, name, pbag),
            ContextKind::ElementCreation(ref parent) => {
                let parent = parent.clone();
                self.element_creation_start_element(Some(&parent), handle, name, pbag)
            }
            ContextKind::Style(_) => self.inside_style_start_element(name),
            ContextKind::UnsupportedStyleChild => self.inside_style_start_element(name),
            ContextKind::XInclude => self.xinclude_start_element(handle, name, pbag),
        };

        self.push_context(new_ctx);
    }

    pub fn end_element(&mut self, handle: *mut RsvgHandle, name: &str) {
        // We can unwrap since start_element() always adds a context to the stack
        let top = self.context_stack.pop().unwrap();

        let is_root = if let ContextKind::Start = top.kind {
            true
        } else {
            false
        };

        let context = mem::replace(&mut self.context, top);

        assert!(context.element_name == name);

        match context.kind {
            ContextKind::Start => panic!("end_element: XML handler stack is empty!?"),
            ContextKind::ElementCreation(node) => {
                self.element_creation_end_element(is_root, node, handle)
            }
            ContextKind::Style(style_ctx) => self.style_end_element(style_ctx, handle),
            ContextKind::UnsupportedStyleChild => (),
            ContextKind::XInclude => self.xinclude_end_element(handle, name),
        }
    }

    pub fn characters(&mut self, text: &str) {
        match self.context.kind {
            ContextKind::Start => panic!("characters: XML handler stack is empty!?"),
            ContextKind::ElementCreation(ref parent) => {
                self.element_creation_characters(parent, text)
            }
            ContextKind::Style(ref mut style_ctx) => style_characters(style_ctx, text),
            ContextKind::UnsupportedStyleChild => (),
            ContextKind::XInclude => self.xinclude_characters(text),
        }
    }

    fn element_creation_start_element(
        &self,
        parent: Option<&Rc<Node>>,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Context {
        match name {
            "include" => unimplemented!(),
            "style" => self.style_start_element(name, pbag),
            _ => {
                let node = self.create_node(parent, handle, name, pbag);

                Context {
                    element_name: name.to_string(),
                    kind: ContextKind::ElementCreation(node),
                }
            }
        }
    }

    fn element_creation_end_element(
        &mut self,
        is_root: bool,
        node: Rc<Node>,
        handle: *mut RsvgHandle,
    ) {
        // The "svg" node is special; it parses its style attributes
        // here, not during element creation.
        if node.get_type() == NodeType::Svg {
            node.with_impl(|svg: &NodeSvg| {
                svg.set_delayed_style(&node, handle);
            });
        }

        if is_root {
            self.set_root(&node);
        }
    }

    fn element_creation_characters(&self, node: &Rc<Node>, text: &str) {
        if text.len() != 0 && node.accept_chars() {
            let chars_node = if let Some(child) = node.find_last_chars_child() {
                child
            } else {
                let child = node_new(
                    NodeType::Chars,
                    Some(node),
                    "rsvg-chars",
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

    fn create_node(
        &self,
        parent: Option<&Rc<Node>>,
        handle: *mut RsvgHandle,
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
            new_node.set_style(handle, pbag);
        }

        new_node.set_overridden_properties();

        new_node
    }

    fn style_start_element(&self, name: &str, pbag: &PropertyBag) -> Context {
        // FIXME: See these:
        //
        // https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute
        // https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute
        //
        // If the "type" attribute is not present, we should fallback to the
        // "contentStyleType" attribute of the svg element, which in turn
        // defaults to "text/css".
        //
        // See where is_text_css is used to see where we parse the contents
        // of the style element.

        let mut is_text_css = true;

        for (_key, attr, value) in pbag.iter() {
            if attr == Attribute::Type {
                is_text_css = value == "text/css";
            }
        }

        Context {
            element_name: name.to_string(),
            kind: ContextKind::Style(StyleContext {
                is_text_css,
                text: String::new(),
            }),
        }
    }

    fn style_end_element(&mut self, style_ctx: StyleContext, handle: *mut RsvgHandle) {
        if style_ctx.is_text_css {
            css::parse_into_handle(handle, &style_ctx.text);
        }
    }

    fn inside_style_start_element(&self, name: &str) -> Context {
        // We are already inside a <style> element, and we don't support
        // elements in there.  Just push a state that we will ignore.

        Context {
            element_name: name.to_string(),
            kind: ContextKind::UnsupportedStyleChild,
        }
    }

    fn xinclude_start_element(
        &mut self,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Context {
        unimplemented!();
    }

    fn xinclude_end_element(&mut self, handle: *mut RsvgHandle, name: &str) {
        unimplemented!();
    }

    fn xinclude_characters(&mut self, text: &str) {
        unimplemented!();
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
