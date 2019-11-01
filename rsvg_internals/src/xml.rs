use crate::xml_rs::{reader::XmlEvent, ParserConfig};
use encoding::label::encoding_from_whatwg_label;
use encoding::DecoderTrap;
use libc;
use markup5ever::{ExpandedName, LocalName, Namespace, QualName};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::str;

use crate::allowed_url::AllowedUrl;
use crate::document::{Document, DocumentBuilder};
use crate::error::LoadingError;
use crate::io::{self, get_input_stream_for_loading};
use crate::limits::MAX_LOADED_ELEMENTS;
use crate::node::{NodeType, RsvgNode};
use crate::property_bag::PropertyBag;
use crate::style::Style;
use crate::xml2_load::{ParseFromStreamError, Xml2Parser};

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
    FatalError(ParseFromStreamError),
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

// Creates an ExpandedName from the XInclude namespace and a local_name
//
// The markup5ever crate doesn't have built-in namespaces for XInclude,
// so we make our own.
macro_rules! xinclude_name {
    ($local_name:expr) => {
        ExpandedName {
            ns: &Namespace::from("http://www.w3.org/2001/XInclude"),
            local: &LocalName::from($local_name),
        }
    };
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
struct XmlStateInner {
    weak: Option<Weak<XmlState>>,
    document_builder: Option<DocumentBuilder>,
    num_loaded_elements: usize,
    context_stack: Vec<Context>,
    current_node: Option<RsvgNode>,

    entities: HashMap<String, XmlEntityPtr>,
}

pub struct XmlState {
    inner: RefCell<XmlStateInner>,

    unlimited_size: bool,
}

/// Errors returned from XmlState::acquire()
///
/// These follow the terminology from https://www.w3.org/TR/xinclude/#terminology
enum AcquireError {
    /// Resource could not be acquired (file not found), or I/O error.
    /// In this case, the `xi:fallback` can be used if present.
    ResourceError,

    /// Resource could not be parsed/decoded
    FatalError(String),
}

impl XmlStateInner {
    fn context(&self) -> Context {
        // We can unwrap since the stack is never empty
        self.context_stack.last().unwrap().clone()
    }
}

impl XmlState {
    fn new(document_builder: DocumentBuilder, unlimited_size: bool) -> XmlState {
        XmlState {
            inner: RefCell::new(XmlStateInner {
                weak: None,
                document_builder: Some(document_builder),
                num_loaded_elements: 0,
                context_stack: vec![Context::Start],
                current_node: None,
                entities: HashMap::new(),
            }),

            unlimited_size,
        }
    }

    fn check_last_error(&self) -> Result<(), ParseFromStreamError> {
        let inner = self.inner.borrow();

        match inner.context() {
            Context::FatalError(e) => Err(e),
            _ => Ok(()),
        }
    }

    fn check_limits(&self) -> Result<(), ()> {
        if self.inner.borrow().num_loaded_elements > MAX_LOADED_ELEMENTS {
            self.error(ParseFromStreamError::XmlParseError(format!(
                "cannot load more than {} XML elements",
                MAX_LOADED_ELEMENTS
            )));
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn start_element(&self, name: QualName, pbag: &PropertyBag) -> Result<(), ()> {
        self.check_limits()?;

        let context = self.inner.borrow().context();

        if let Context::FatalError(_) = context {
            return Err(());
        }

        self.inner.borrow_mut().num_loaded_elements += 1;

        let new_context = match context {
            Context::Start => self.element_creation_start_element(&name, pbag),
            Context::ElementCreation => self.element_creation_start_element(&name, pbag),
            Context::XInclude(ref ctx) => self.inside_xinclude_start_element(&ctx, &name),
            Context::UnsupportedXIncludeChild => self.unsupported_xinclude_start_element(&name),
            Context::XIncludeFallback(ref ctx) => {
                self.xinclude_fallback_start_element(&ctx, &name, pbag)
            }

            Context::FatalError(_) => unreachable!(),
        };

        self.inner.borrow_mut().context_stack.push(new_context);

        Ok(())
    }

    pub fn end_element(&self, _name: QualName) {
        let context = self.inner.borrow().context();

        match context {
            Context::Start => panic!("end_element: XML handler stack is empty!?"),
            Context::ElementCreation => self.element_creation_end_element(),
            Context::XInclude(_) => (),
            Context::UnsupportedXIncludeChild => (),
            Context::XIncludeFallback(_) => (),
            Context::FatalError(_) => return,
        }

        // We can unwrap since start_element() always adds a context to the stack
        self.inner.borrow_mut().context_stack.pop().unwrap();
    }

    pub fn characters(&self, text: &str) {
        let context = self.inner.borrow().context();

        match context {
            // This is character data before the first element, i.e. something like
            //  <?xml version="1.0" encoding="UTF-8"?><svg xmlns="http://www.w3.org/2000/svg"/>
            // ^ note the space here
            // libxml2 is not finished reading the file yet; it will emit an error
            // on its own when it finishes.  So, ignore this condition.
            Context::Start => (),

            Context::ElementCreation => self.element_creation_characters(text),
            Context::XInclude(_) => (),
            Context::UnsupportedXIncludeChild => (),
            Context::XIncludeFallback(ref ctx) => self.xinclude_fallback_characters(&ctx, text),
            Context::FatalError(_) => (),
        }
    }

    pub fn processing_instruction(&self, target: &str, data: &str) {
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

            let mut inner = self.inner.borrow_mut();
            inner
                .document_builder
                .as_mut()
                .unwrap()
                .append_stylesheet(alternate, type_, href);
        } else {
            self.error(ParseFromStreamError::XmlParseError(String::from(
                "invalid processing instruction data in xml-stylesheet",
            )));
        }
    }

    pub fn error(&self, e: ParseFromStreamError) {
        self.inner
            .borrow_mut()
            .context_stack
            .push(Context::FatalError(e));
    }

    pub fn entity_lookup(&self, entity_name: &str) -> Option<XmlEntityPtr> {
        self.inner.borrow().entities.get(entity_name).copied()
    }

    pub fn entity_insert(&self, entity_name: &str, entity: XmlEntityPtr) {
        let mut inner = self.inner.borrow_mut();

        let old_value = inner.entities.insert(entity_name.to_string(), entity);

        if let Some(v) = old_value {
            unsafe {
                xmlFreeNode(v);
            }
        }
    }

    fn element_creation_start_element(&self, name: &QualName, pbag: &PropertyBag) -> Context {
        if name.expanded() == xinclude_name!("include") {
            self.xinclude_start_element(name, pbag)
        } else {
            let mut inner = self.inner.borrow_mut();

            let parent = inner.current_node.clone();
            let node = inner
                .document_builder
                .as_mut()
                .unwrap()
                .append_element(name, pbag, parent);
            inner.current_node = Some(node);

            Context::ElementCreation
        }
    }

    fn element_creation_end_element(&self) {
        let mut inner = self.inner.borrow_mut();

        let node = inner.current_node.take().unwrap();

        if node.borrow().get_type() == NodeType::Style {
            let css_data = node.borrow().get_impl::<Style>().get_css(&node);
            inner
                .document_builder
                .as_mut()
                .unwrap()
                .parse_css(&css_data);
        }

        inner.current_node = node.parent();
    }

    fn element_creation_characters(&self, text: &str) {
        let mut inner = self.inner.borrow_mut();

        let mut parent = inner.current_node.clone().unwrap();
        inner
            .document_builder
            .as_mut()
            .unwrap()
            .append_characters(text, &mut parent);
    }

    fn xinclude_start_element(&self, _name: &QualName, pbag: &PropertyBag) -> Context {
        let mut href = None;
        let mut parse = None;
        let mut encoding = None;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                ref n if *n == xinclude_name!("href") => href = Some(value),
                ref n if *n == xinclude_name!("parse") => parse = Some(value),
                ref n if *n == xinclude_name!("encoding") => encoding = Some(value),
                _ => (),
            }
        }

        let need_fallback = match self.acquire(href, parse, encoding) {
            Ok(()) => false,
            Err(AcquireError::ResourceError) => true,
            Err(AcquireError::FatalError(s)) => {
                return Context::FatalError(ParseFromStreamError::XmlParseError(s))
            }
        };

        Context::XInclude(XIncludeContext { need_fallback })
    }

    fn inside_xinclude_start_element(&self, ctx: &XIncludeContext, name: &QualName) -> Context {
        if name.expanded() == xinclude_name!("fallback") {
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
        &self,
        ctx: &XIncludeContext,
        name: &QualName,
        pbag: &PropertyBag,
    ) -> Context {
        if ctx.need_fallback {
            if name.expanded() == xinclude_name!("include") {
                self.xinclude_start_element(name, pbag)
            } else {
                self.element_creation_start_element(name, pbag)
            }
        } else {
            Context::UnsupportedXIncludeChild
        }
    }

    fn xinclude_fallback_characters(&self, ctx: &XIncludeContext, text: &str) {
        if ctx.need_fallback && self.inner.borrow().current_node.is_some() {
            // We test for is_some() because with a bad "SVG" file like this:
            //
            //    <xi:include href="blah"><xi:fallback>foo</xi:fallback></xi:include>
            //
            // at the point we get "foo" here, there is no current_node because
            // no nodes have been created before the xi:include.
            self.element_creation_characters(text);
        }
    }

    fn acquire(
        &self,
        href: Option<&str>,
        parse: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<(), AcquireError> {
        if let Some(href) = href {
            let aurl = self
                .inner
                .borrow()
                .document_builder
                .as_ref()
                .unwrap()
                .resolve_href(href)
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

                Some(v) => Err(AcquireError::FatalError(format!(
                    "unknown 'parse' attribute value: \"{}\"",
                    v
                ))),
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

    fn acquire_text(&self, aurl: &AllowedUrl, encoding: Option<&str>) -> Result<(), AcquireError> {
        let binary = io::acquire_data(aurl, None).map_err(|e| {
            rsvg_log!("could not acquire \"{}\": {}", aurl, e);
            AcquireError::ResourceError
        })?;

        let encoding = encoding.unwrap_or("utf-8");

        let encoder = encoding_from_whatwg_label(encoding).ok_or_else(|| {
            AcquireError::FatalError(format!(
                "unknown encoding \"{}\" for \"{}\"",
                encoding, aurl
            ))
        })?;

        let utf8_data = encoder
            .decode(&binary.data, DecoderTrap::Strict)
            .map_err(|e| {
                AcquireError::FatalError(format!(
                    "could not convert contents of \"{}\" from character encoding \"{}\": {}",
                    aurl, encoding, e
                ))
            })?;

        self.element_creation_characters(&utf8_data);
        Ok(())
    }

    fn acquire_xml(&self, aurl: &AllowedUrl) -> Result<(), AcquireError> {
        // FIXME: distinguish between "file not found" and "invalid XML"

        let stream = io::acquire_stream(aurl, None).map_err(|e| match e {
            LoadingError::BadDataUrl => {
                AcquireError::FatalError(String::from("malformed data: URL"))
            }
            _ => AcquireError::ResourceError,
        })?;

        // FIXME: pass a cancellable
        self.parse_from_stream(&stream, None).map_err(|e| match e {
            ParseFromStreamError::CouldNotCreateXmlParser => {
                AcquireError::FatalError(String::from("could not create XML parser"))
            }
            ParseFromStreamError::IoError(_) => AcquireError::ResourceError,
            ParseFromStreamError::XmlParseError(s) => AcquireError::FatalError(s),
        })
    }

    // Parses XML from a stream into an XmlState.
    //
    // This can be called "in the middle" of an XmlState's processing status,
    // for example, when including another XML file via xi:include.
    fn parse_from_stream(
        &self,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<(), ParseFromStreamError> {
        let strong = self
            .inner
            .borrow()
            .weak
            .as_ref()
            .unwrap()
            .upgrade()
            .unwrap();
        Xml2Parser::from_stream(strong, self.unlimited_size, stream, cancellable)
            .and_then(|parser| parser.parse())
            .and_then(|_: ()| self.check_last_error())
    }

    fn unsupported_xinclude_start_element(&self, _name: &QualName) -> Context {
        Context::UnsupportedXIncludeChild
    }

    fn build_document(
        &self,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Document, LoadingError> {
        self.parse_from_stream(stream, cancellable)?;

        self.inner
            .borrow_mut()
            .document_builder
            .take()
            .unwrap()
            .build()
    }
}

impl Drop for XmlState {
    fn drop(&mut self) {
        unsafe {
            let mut inner = self.inner.borrow_mut();

            for (_key, entity) in inner.entities.drain() {
                xmlFreeNode(entity);
            }
        }
    }
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
        match event {
            Ok(XmlEvent::StartElement { attributes, .. }) => {
                return Ok(attributes
                    .iter()
                    .map(|att| (att.name.local_name.clone(), att.value.clone()))
                    .collect());
            }
            Err(_) => return Err(()),
            _ => (),
        }
    }

    unreachable!();
}

pub fn xml_load_from_possibly_compressed_stream(
    document_builder: DocumentBuilder,
    unlimited_size: bool,
    stream: &gio::InputStream,
    cancellable: Option<&gio::Cancellable>,
) -> Result<Document, LoadingError> {
    let state = Rc::new(XmlState::new(document_builder, unlimited_size));

    state.inner.borrow_mut().weak = Some(Rc::downgrade(&state));

    let stream =
        get_input_stream_for_loading(stream, cancellable).map_err(ParseFromStreamError::IoError)?;

    state.build_document(&stream, cancellable)
}
