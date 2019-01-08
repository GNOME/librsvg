use encoding::label::encoding_from_whatwg_label;
use encoding::DecoderTrap;
use glib::translate::*;
use libc;
use std::collections::HashMap;
use std::rc::Rc;
use std::str;
use xml_rs::{reader::XmlEvent, ParserConfig};

use allowed_url::AllowedUrl;
use attributes::Attribute;
use create_node::create_node_and_register_id;
use css::{self, CssStyles};
use defs::Defs;
use error::LoadingError;
use handle::{self, RsvgHandle};
use io;
use node::{node_new, Node, NodeType, RsvgNode};
use property_bag::PropertyBag;
use structure::NodeSvg;
use style::NodeStyle;
use svg::Svg;
use text::NodeChars;
use tree::Tree;
use xml2_load::{xml_state_parse_from_stream, ParseFromStreamError};

#[derive(Clone)]
enum Context {
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

// This is to hold an xmlEntityPtr from libxml2; we just hold an opaque pointer
// that is freed in impl Drop for XmlState
type XmlEntityPtr = *mut libc::c_void;

extern "C" {
    // The original function takes an xmlNodePtr, but that is compatible
    // with xmlEntityPtr for the purposes of this function.
    fn xmlFreeNode(node: XmlEntityPtr);
}

/// Holds the state used for XML processing
///
/// These methods are called when an XML event is parsed out of the XML stream: `start_element`,
/// `end_element`, `characters`.
///
/// When an element starts, we push a corresponding `Context` into the `context_stack`.  Within
/// that context, all XML events will be forwarded to it, and processed in one of the `XmlHandler`
/// trait objects. Normally the context refers to a `NodeCreationContext` implementation which is
/// what creates normal graphical elements.
pub struct XmlState {
    tree: Option<Tree>,
    defs: Option<Defs>,
    ids: Option<HashMap<String, RsvgNode>>,
    css_styles: Option<CssStyles>,
    context_stack: Vec<Context>,
    current_node: Option<Rc<Node>>,

    entities: HashMap<String, XmlEntityPtr>,

    handle: *mut RsvgHandle,
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
    pub fn new(handle: *mut RsvgHandle) -> XmlState {
        XmlState {
            tree: None,
            defs: Some(Defs::new()),
            ids: Some(HashMap::new()),
            css_styles: Some(CssStyles::new()),
            context_stack: vec![Context::Start],
            current_node: None,
            entities: HashMap::new(),
            handle,
        }
    }

    fn set_root(&mut self, root: &Rc<Node>) {
        if self.tree.is_some() {
            panic!("The tree root has already been set");
        }

        self.tree = Some(Tree::new(root));
    }

    pub fn validate_tree(&self) -> Result<(), LoadingError> {
        if let Some(ref tree) = self.tree {
            if tree.root_is_svg() {
                Ok(())
            } else {
                Err(LoadingError::RootElementIsNotSvg)
            }
        } else {
            Err(LoadingError::SvgHasNoElements)
        }
    }

    pub fn steal_result(&mut self) -> Svg {
        Svg::new(
            self.handle,
            self.tree.take().unwrap(),
            self.defs.take().unwrap(),
            self.ids.take().unwrap(),
            self.css_styles.take().unwrap(),
        )
    }

    fn context(&self) -> Context {
        // We can unwrap since the stack is never empty
        self.context_stack.last().unwrap().clone()
    }

    pub fn start_element(&mut self, name: &str, pbag: &PropertyBag) {
        let context = self.context();

        if let Context::FatalError = context {
            return;
        }

        // FIXME: we should deal with namespaces at some point
        let name = skip_namespace(name);

        let new_context = match context {
            Context::Start => self.element_creation_start_element(name, pbag),
            Context::ElementCreation => self.element_creation_start_element(name, pbag),
            Context::XInclude(ref ctx) => self.inside_xinclude_start_element(&ctx, name),
            Context::UnsupportedXIncludeChild => self.unsupported_xinclude_start_element(name),
            Context::XIncludeFallback(ref ctx) => {
                self.xinclude_fallback_start_element(&ctx, name, pbag)
            }

            Context::FatalError => unreachable!(),
        };

        self.context_stack.push(new_context);
    }

    pub fn end_element(&mut self, _name: &str) {
        let context = self.context();

        match context {
            Context::Start => panic!("end_element: XML handler stack is empty!?"),
            Context::ElementCreation => self.element_creation_end_element(),
            Context::XInclude(_) => (),
            Context::UnsupportedXIncludeChild => (),
            Context::XIncludeFallback(_) => (),
            Context::FatalError => return,
        }

        // We can unwrap since start_element() always adds a context to the stack
        self.context_stack.pop().unwrap();
    }

    pub fn characters(&mut self, text: &str) {
        let context = self.context();

        match context {
            Context::Start => panic!("characters: XML handler stack is empty!?"),
            Context::ElementCreation => self.element_creation_characters(text),
            Context::XInclude(_) => (),
            Context::UnsupportedXIncludeChild => (),
            Context::XIncludeFallback(ref ctx) => self.xinclude_fallback_characters(&ctx, text),
            Context::FatalError => return,
        }
    }

    pub fn processing_instruction(&mut self, target: &str, data: &str) {
        if target != "xml-stylesheet" {
            return;
        }

        if let Ok(pairs) = parse_xml_stylesheet_processing_instruction(data) {
            let mut alternate = None;
            let mut type_ = None;
            let mut href = None;

            for (att, value) in pairs {
                match att.as_str() {
                    "alternate" => alternate = Some(value),
                    "type" => type_ = Some(value),
                    "href" => href = Some(value),
                    _ => (),
                }
            }

            if (alternate == None || alternate.as_ref().map(String::as_str) == Some("no"))
                && type_.as_ref().map(String::as_str) == Some("text/css")
                && href.is_some()
            {
                if let Ok(aurl) = AllowedUrl::from_href(
                    &href.unwrap(),
                    handle::get_base_url(self.handle).as_ref(),
                ) {
                    // FIXME: handle CSS errors
                    let _ = handle::load_css(self.css_styles.as_mut().unwrap(), &aurl);
                } else {
                    self.error("disallowed URL in xml-stylesheet");
                }
            }
        } else {
            self.error("invalid processing instruction data in xml-stylesheet");
        }
    }

    pub fn error(&mut self, msg: &str) {
        // FIXME: aggregate the errors and expose them to the public result

        rsvg_log!("XML error: {}", msg);

        self.context_stack.push(Context::FatalError);
    }

    pub fn entity_lookup(&self, entity_name: &str) -> Option<XmlEntityPtr> {
        self.entities.get(entity_name).map(|v| *v)
    }

    pub fn entity_insert(&mut self, entity_name: &str, entity: XmlEntityPtr) {
        let old_value = self.entities.insert(entity_name.to_string(), entity);

        if let Some(v) = old_value {
            unsafe {
                xmlFreeNode(v);
            }
        }
    }

    fn element_creation_start_element(&mut self, name: &str, pbag: &PropertyBag) -> Context {
        match name {
            "include" => self.xinclude_start_element(name, pbag),
            _ => {
                let parent = self.current_node.clone();
                let node = self.create_node(parent.as_ref(), name, pbag);
                if self.current_node.is_none() {
                    self.set_root(&node);
                }
                self.current_node = Some(node);

                Context::ElementCreation
            }
        }
    }

    fn element_creation_end_element(&mut self) {
        let node = self.current_node.take().unwrap();

        // The "svg" node is special; it parses its style attributes
        // here, not during element creation.
        if node.get_type() == NodeType::Svg {
            node.with_impl(|svg: &NodeSvg| {
                svg.set_delayed_style(&node, self.css_styles.as_ref().unwrap());
            });
        }

        if node.get_type() == NodeType::Style {
            let css_data = node.with_impl(|style: &NodeStyle| style.get_css(&node));

            css::parse_into_css_styles(
                self.css_styles.as_mut().unwrap(),
                handle::get_base_url(self.handle).clone(),
                &css_data,
            );
        }

        self.current_node = node.get_parent();
    }

    fn element_creation_characters(&self, text: &str) {
        let node = self.current_node.as_ref().unwrap();

        if text.len() != 0 {
            let chars_node = if let Some(child) = node.find_last_chars_child() {
                child
            } else {
                let child = node_new(
                    NodeType::Chars,
                    Some(node),
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
        name: &str,
        pbag: &PropertyBag,
    ) -> Rc<Node> {
        let ids = self.ids.as_mut().unwrap();

        let new_node = create_node_and_register_id(name, parent, pbag, ids);

        if let Some(parent) = parent {
            parent.add_child(&new_node);
        }

        new_node.set_atts(&new_node, self.handle, pbag);

        // The "svg" node is special; it will parse its style attributes
        // until the end, in standard_element_end().
        if new_node.get_type() != NodeType::Svg {
            new_node.set_style(self.css_styles.as_ref().unwrap(), pbag);
        }

        new_node.set_overridden_properties();

        new_node
    }

    fn xinclude_start_element(&mut self, _name: &str, pbag: &PropertyBag) -> Context {
        let mut href = None;
        let mut parse = None;
        let mut encoding = None;

        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::Href => href = Some(value),
                Attribute::Parse => parse = Some(value),
                Attribute::Encoding => encoding = Some(value),
                _ => (),
            }
        }

        let need_fallback = match self.acquire(href, parse, encoding) {
            Ok(()) => false,
            Err(AcquireError::ResourceError) => true,
            Err(AcquireError::FatalError) => return Context::FatalError,
        };

        Context::XInclude(XIncludeContext { need_fallback })
    }

    fn inside_xinclude_start_element(&self, ctx: &XIncludeContext, name: &str) -> Context {
        // FIXME: we aren't using the xi: namespace
        if name == "fallback" {
            Context::XIncludeFallback(ctx.clone())
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
        name: &str,
        pbag: &PropertyBag,
    ) -> Context {
        if ctx.need_fallback {
            // FIXME: we aren't using the xi: namespace
            if name == "include" {
                self.xinclude_start_element(name, pbag)
            } else {
                self.element_creation_start_element(name, pbag)
            }
        } else {
            Context::UnsupportedXIncludeChild
        }
    }

    fn xinclude_fallback_characters(&mut self, ctx: &XIncludeContext, text: &str) {
        if ctx.need_fallback {
            self.element_creation_characters(text);
        }
    }

    fn acquire(
        &mut self,
        href: Option<&str>,
        parse: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<(), AcquireError> {
        if let Some(href) = href {
            let aurl = AllowedUrl::from_href(href, handle::get_base_url(self.handle).as_ref())
                .map_err(|e| {
                    // FIXME: should AlloweUrlError::HrefParseError be a fatal error,
                    // not a resource error?
                    rsvg_log!("could not acquire \"{}\": {}", href, e);
                    AcquireError::ResourceError
                })?;

            // https://www.w3.org/TR/xinclude/#include_element
            //
            // "When omitted, the value of "xml" is implied (even in
            // the absence of a default value declaration). Values
            // other than "xml" and "text" are a fatal error."
            match parse {
                None | Some("xml") => self.acquire_xml(&aurl),

                Some("text") => self.acquire_text(&aurl, encoding),

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
        aurl: &AllowedUrl,
        encoding: Option<&str>,
    ) -> Result<(), AcquireError> {
        let binary = io::acquire_data(aurl, None).map_err(|e| {
            rsvg_log!("could not acquire \"{}\": {}", aurl.url(), e);
            AcquireError::ResourceError
        })?;

        let encoding = encoding.unwrap_or("utf-8");

        let encoder = encoding_from_whatwg_label(encoding).ok_or_else(|| {
            rsvg_log!("unknown encoding \"{}\" for \"{}\"", encoding, aurl.url());
            AcquireError::FatalError
        })?;

        let utf8_data = encoder
            .decode(&binary.data, DecoderTrap::Strict)
            .map_err(|e| {
                rsvg_log!(
                    "could not convert contents of \"{}\" from character encoding \"{}\": {}",
                    aurl.url(),
                    encoding,
                    e
                );
                AcquireError::FatalError
            })?;

        self.element_creation_characters(&utf8_data);
        Ok(())
    }

    fn acquire_xml(&mut self, aurl: &AllowedUrl) -> Result<(), AcquireError> {
        // FIXME: distinguish between "file not found" and "invalid XML"

        let stream = io::acquire_stream(aurl, None).map_err(|e| match e {
            LoadingError::BadDataUrl => AcquireError::FatalError,
            _ => AcquireError::ResourceError,
        })?;

        let load_options = handle::get_load_options(self.handle);

        // FIXME: pass a cancellable
        xml_state_parse_from_stream(self, &load_options, stream, None).map_err(|e| match e {
            ParseFromStreamError::CouldNotCreateXmlParser => AcquireError::FatalError,
            ParseFromStreamError::IoError(_) => AcquireError::ResourceError,
            ParseFromStreamError::XmlParseError(_) => AcquireError::FatalError,
        })
    }

    fn unsupported_xinclude_start_element(&self, _name: &str) -> Context {
        Context::UnsupportedXIncludeChild
    }
}

impl Drop for XmlState {
    fn drop(&mut self) {
        unsafe {
            for (_key, entity) in self.entities.drain() {
                xmlFreeNode(entity);
            }
        }
    }
}

fn skip_namespace(s: &str) -> &str {
    s.find(':').map_or(s, |pos| &s[pos + 1..])
}

// https://www.w3.org/TR/xml-stylesheet/
//
// The syntax for the xml-stylesheet processing instruction we support
// is this:
//
//   <?xml-stylesheet href="uri" alternate="no" type="text/css"?>
//
// XML parsers just feed us the raw data after the target name
// ("xml-stylesheet"), so we'll create a mini-parser with a hackish
// element just to extract the data as attributes.
fn parse_xml_stylesheet_processing_instruction(data: &str) -> Result<Vec<(String, String)>, ()> {
    let xml_str = format!("<rsvg-hack {} />\n", data);

    let mut buf = xml_str.as_bytes();

    let reader = ParserConfig::new().create_reader(&mut buf);

    for event in reader {
        if let Ok(event) = event {
            match event {
                XmlEvent::StartElement { attributes, .. } => {
                    return Ok(attributes
                        .iter()
                        .map(|att| (att.name.local_name.clone(), att.value.clone()))
                        .collect());
                }

                _ => (),
            }
        } else {
            return Err(());
        }
    }

    unreachable!();
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_xml_state_error(xml: *mut XmlState, msg: *const libc::c_char) {
    assert!(!xml.is_null());
    let xml = &mut *xml;

    assert!(!msg.is_null());
    // Unlike the functions that take UTF-8 validated strings from
    // libxml2, I don't trust error messages to be validated.
    let msg: String = from_glib_none(msg);

    xml.error(&msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_namespaces() {
        assert_eq!(skip_namespace("foo"), "foo");
        assert_eq!(skip_namespace("foo:bar"), "bar");
        assert_eq!(skip_namespace("foo:bar:baz"), "bar:baz");
    }
}
