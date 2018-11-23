use encoding::label::encoding_from_whatwg_label;
use encoding::DecoderTrap;
use glib::translate::*;
use libc;
use std;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::str;

use attributes::Attribute;
use css;
use defs::{Defs, RsvgDefs};
use handle::{self, RsvgHandle};
use load::rsvg_load_new_node;
use node::{node_new, Node, NodeType};
use property_bag::PropertyBag;
use structure::NodeSvg;
use style::NodeStyle;
use text::NodeChars;
use tree::{RsvgTree, Tree};
use util::utf8_cstr;

#[derive(Clone)]
enum ContextKind {
    // Starting state
    Start,

    // Creating nodes for elements under the current node
    ElementCreation,

    // Inside <xi:include>
    XInclude(XIncludeContext),

    // An unsupported element inside a <xi:include> context, to be ignored
    UnsupportedXIncludeChild,

    // Insie <xi::fallback>
    XIncludeFallback(XIncludeContext),

    // An XML parsing error was found.  We will no-op upon any further XML events.
    FatalError,
}

#[derive(Clone)]
struct XIncludeContext {
    need_fallback: bool,
}

/// A concrete parsing context for a surrounding `element_name` and its XML event handlers
#[derive(Clone)]
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

    fn fatal_error() -> Context {
        Context {
            element_name: "".to_string(),
            kind: ContextKind::FatalError,
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
struct XmlState {
    tree: Option<Box<Tree>>,
    defs: Option<Defs>,
    context: Context,
    context_stack: Vec<Context>,
    current_node: Option<Rc<Node>>,
}

/// Errors returned from XmlState::acquire()
///
/// These follow the terminology from https://www.w3.org/TR/xinclude/#terminology
enum AcquireError {
    /// Resource could not be acquired (file not found), or I/O error.
    /// In this case, the `xi:fallback` can be used if present.
    ResourceError,

    /// Resource could not be parsed/decoded
    FatalError,
}

impl XmlState {
    fn new() -> XmlState {
        XmlState {
            tree: None,
            defs: Some(Defs::new()),
            context: Context::empty(),
            context_stack: Vec::new(),
            current_node: None,
        }
    }

    pub fn set_root(&mut self, root: &Rc<Node>) {
        if self.tree.is_some() {
            panic!("The tree root has already been set");
        }

        self.tree = Some(Box::new(Tree::new(root)));
    }

    pub fn steal_result(&mut self) -> (Option<Box<Tree>>, Box<Defs>) {
        (self.tree.take(), Box::new(self.defs.take().unwrap()))
    }

    fn push_context(&mut self, ctx: Context) {
        let top = mem::replace(&mut self.context, ctx);
        self.context_stack.push(top);
    }

    pub fn start_element(&mut self, handle: *mut RsvgHandle, name: &str, pbag: &PropertyBag) {
        let context = self.context.clone();

        if let ContextKind::FatalError = context.kind {
            return;
        }

        let new_context = match context.kind {
            ContextKind::Start => self.element_creation_start_element(handle, name, pbag),
            ContextKind::ElementCreation => self.element_creation_start_element(handle, name, pbag),
            ContextKind::XInclude(ref ctx) => self.inside_xinclude_start_element(&ctx, name),
            ContextKind::UnsupportedXIncludeChild => self.unsupported_xinclude_start_element(name),
            ContextKind::XIncludeFallback(ref ctx) => {
                self.xinclude_fallback_start_element(&ctx, handle, name, pbag)
            }

            ContextKind::FatalError => unreachable!(),
        };

        self.push_context(new_context);
    }

    pub fn end_element(&mut self, handle: *mut RsvgHandle, name: &str) {
        let context = self.context.clone();

        if let ContextKind::FatalError = context.kind {
            return;
        }

        assert!(context.element_name == name);

        match context.kind {
            ContextKind::Start => panic!("end_element: XML handler stack is empty!?"),
            ContextKind::ElementCreation => self.element_creation_end_element(handle),
            ContextKind::XInclude(_) => (),
            ContextKind::UnsupportedXIncludeChild => (),
            ContextKind::XIncludeFallback(_) => (),
            ContextKind::FatalError => unreachable!(),
        }

        // We can unwrap since start_element() always adds a context to the stack
        self.context = self.context_stack.pop().unwrap();
    }

    pub fn characters(&mut self, text: &str) {
        let context = self.context.clone();

        if let ContextKind::FatalError = context.kind {
            return;
        }

        match context.kind {
            ContextKind::Start => panic!("characters: XML handler stack is empty!?"),
            ContextKind::ElementCreation => self.element_creation_characters(text),
            ContextKind::XInclude(_) => (),
            ContextKind::UnsupportedXIncludeChild => (),
            ContextKind::XIncludeFallback(ref ctx) => self.xinclude_fallback_characters(&ctx, text),
            ContextKind::FatalError => unreachable!(),
        }
    }

    pub fn error(&mut self, msg: &str) {
        // FIXME: aggregate the errors and expose them to the public result

        rsvg_log!("XML error: {}", msg);

        self.push_context(Context::fatal_error());
    }

    fn element_creation_start_element(
        &mut self,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Context {
        match name {
            "include" => self.xinclude_start_element(handle, name, pbag),
            _ => {
                let parent = self.current_node.clone();
                let node = self.create_node(parent.as_ref(), handle, name, pbag);
                if self.current_node.is_none() {
                    self.set_root(&node);
                }
                self.current_node = Some(node);

                Context {
                    element_name: name.to_string(),
                    kind: ContextKind::ElementCreation,
                }
            }
        }
    }

    fn element_creation_end_element(&mut self, handle: *mut RsvgHandle) {
        let node = self.current_node.take().unwrap();

        // The "svg" node is special; it parses its style attributes
        // here, not during element creation.
        if node.get_type() == NodeType::Svg {
            node.with_impl(|svg: &NodeSvg| {
                let css_styles = handle::get_css_styles(handle);
                svg.set_delayed_style(&node, css_styles);
            });
        }

        if node.get_type() == NodeType::Style {
            let css_data = node.with_impl(|style: &NodeStyle| style.get_css(&node));

            css::parse_into_handle(handle, &css_data);
        }

        self.current_node = node.get_parent();
    }

    fn element_creation_characters(&self, text: &str) {
        let node = self.current_node.as_ref().unwrap();

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
        &mut self,
        parent: Option<&Rc<Node>>,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Rc<Node> {
        let defs = self.defs.as_mut().unwrap();

        let new_node = rsvg_load_new_node(name, parent, pbag, defs);

        if let Some(parent) = parent {
            parent.add_child(&new_node);
        }

        new_node.set_atts(&new_node, handle, pbag);

        // The "svg" node is special; it will parse its style attributes
        // until the end, in standard_element_end().
        if new_node.get_type() != NodeType::Svg {
            let css_styles = handle::get_css_styles(handle);
            new_node.set_style(css_styles, pbag);
        }

        new_node.set_overridden_properties();

        new_node
    }

    fn xinclude_start_element(
        &mut self,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Context {
        let mut href = None;
        let mut parse = None;
        let mut encoding = None;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Href => href = Some(value),
                Attribute::Parse => parse = Some(value),
                Attribute::Encoding => encoding = Some(value),
                _ => (),
            }
        }

        let need_fallback = match self.acquire(handle, href, parse, encoding) {
            Ok(()) => false,
            Err(AcquireError::ResourceError) => true,
            Err(AcquireError::FatalError) => return Context::fatal_error(),
        };

        Context {
            element_name: name.to_string(),
            kind: ContextKind::XInclude(XIncludeContext { need_fallback }),
        }
    }

    fn inside_xinclude_start_element(&self, ctx: &XIncludeContext, name: &str) -> Context {
        // FIXME: we aren't using the xi: namespace
        if name == "fallback" {
            Context {
                element_name: name.to_string(),
                kind: ContextKind::XIncludeFallback(ctx.clone()),
            }
        } else {
            // https://www.w3.org/TR/xinclude/#include_element
            //
            // "Other content (text, processing instructions,
            // comments, elements not in the XInclude namespace,
            // descendants of child elements) is not constrained by
            // this specification and is ignored by the XInclude
            // processor"

            self.unsupported_xinclude_start_element(name)
        }
    }

    fn xinclude_fallback_start_element(
        &mut self,
        ctx: &XIncludeContext,
        handle: *mut RsvgHandle,
        name: &str,
        pbag: &PropertyBag,
    ) -> Context {
        if ctx.need_fallback {
            // FIXME: we aren't using the xi: namespace
            if name == "include" {
                self.xinclude_start_element(handle, name, pbag)
            } else {
                self.element_creation_start_element(handle, name, pbag)
            }
        } else {
            Context {
                element_name: name.to_string(),
                kind: ContextKind::UnsupportedXIncludeChild,
            }
        }
    }

    fn xinclude_fallback_characters(&mut self, ctx: &XIncludeContext, text: &str) {
        if ctx.need_fallback {
            self.element_creation_characters(text);
        }
    }

    fn acquire(
        &mut self,
        handle: *mut RsvgHandle,
        href: Option<&str>,
        parse: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<(), AcquireError> {
        if let Some(href) = href {
            // https://www.w3.org/TR/xinclude/#include_element
            //
            // "When omitted, the value of "xml" is implied (even in
            // the absence of a default value declaration). Values
            // other than "xml" and "text" are a fatal error."
            match parse {
                None | Some("xml") => self.acquire_xml(handle, href),

                Some("text") => self.acquire_text(handle, href, encoding),

                _ => Err(AcquireError::FatalError),
            }
        } else {
            // The href attribute is not present.  Per
            // https://www.w3.org/TR/xinclude/#include_element we
            // should use the xpointer attribute, but we do not
            // support that yet.  So, we'll just say, "OK" and not
            // actually include anything.
            Ok(())
        }
    }

    fn acquire_text(
        &mut self,
        handle: *mut RsvgHandle,
        href: &str,
        encoding: Option<&str>,
    ) -> Result<(), AcquireError> {
        let binary = handle::acquire_data(handle, href).map_err(|e| {
            rsvg_log!("could not acquire \"{}\": {}", href, e);
            AcquireError::ResourceError
        })?;

        let encoding = encoding.unwrap_or("utf-8");

        let encoder = encoding_from_whatwg_label(encoding).ok_or_else(|| {
            rsvg_log!("unknown encoding \"{}\" for \"{}\"", encoding, href);
            AcquireError::FatalError
        })?;

        let utf8_data = encoder
            .decode(&binary.data, DecoderTrap::Strict)
            .map_err(|e| {
                rsvg_log!(
                    "could not convert contents of \"{}\" from character encoding \"{}\": {}",
                    href,
                    encoding,
                    e
                );
                AcquireError::FatalError
            })?;

        self.element_creation_characters(&utf8_data);
        Ok(())
    }

    fn acquire_xml(&self, handle: *mut RsvgHandle, href: &str) -> Result<(), AcquireError> {
        // FIXME: distinguish between "file not found" and "invalid XML"
        if handle::load_xml_xinclude(handle, href) {
            Ok(())
        } else {
            Err(AcquireError::FatalError)
        }
    }

    fn unsupported_xinclude_start_element(&self, name: &str) -> Context {
        Context {
            element_name: name.to_string(),
            kind: ContextKind::UnsupportedXIncludeChild,
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
pub unsafe extern "C" fn rsvg_xml_state_steal_result(
    xml: *mut RsvgXmlState,
    out_tree: *mut *mut RsvgTree,
    out_defs: *mut *mut RsvgDefs,
) {
    assert!(!xml.is_null());
    assert!(!out_tree.is_null());
    assert!(!out_defs.is_null());

    let xml = &mut *(xml as *mut XmlState);

    let (tree, defs) = xml.steal_result();

    *out_tree = tree
        .map(|tree| Box::into_raw(tree) as *mut RsvgTree)
        .unwrap_or(ptr::null_mut());

    *out_defs = Box::into_raw(defs) as *mut RsvgDefs;
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

#[no_mangle]
pub unsafe extern "C" fn rsvg_xml_state_error(xml: *mut RsvgXmlState, msg: *const libc::c_char) {
    assert!(!xml.is_null());
    let xml = &mut *(xml as *mut XmlState);

    assert!(!msg.is_null());
    // Unlike the functions that take UTF-8 validated strings from
    // libxml2, I don't trust error messages to be validated.
    let msg: String = from_glib_none(msg);

    xml.error(&msg);
}
