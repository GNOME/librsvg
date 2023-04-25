//! Main SVG document structure.

use data_url::mime::Mime;
use gdk_pixbuf::{prelude::PixbufLoaderExt, PixbufLoader};
use markup5ever::QualName;
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::include_str;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use crate::css::{self, Origin, Stylesheet};
use crate::error::{AcquireError, LoadingError, NodeIdError};
use crate::handle::LoadOptions;
use crate::io::{self, BinaryData};
use crate::limits;
use crate::node::{Node, NodeBorrow, NodeData};
use crate::session::Session;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::url_resolver::{AllowedUrl, UrlResolver};
use crate::xml::{xml_load_from_possibly_compressed_stream, Attributes};

static UA_STYLESHEETS: Lazy<Vec<Stylesheet>> = Lazy::new(|| {
    vec![Stylesheet::from_data(
        include_str!("ua.css"),
        &UrlResolver::new(None),
        Origin::UserAgent,
        Session::default(),
    )
    .expect("could not parse user agent stylesheet for librsvg, there's a bug!")]
});

/// Identifier of a node
#[derive(Debug, PartialEq, Clone)]
pub enum NodeId {
    /// element id
    Internal(String),
    /// url, element id
    External(String, String),
}

impl NodeId {
    pub fn parse(href: &str) -> Result<NodeId, NodeIdError> {
        let (url, id) = match href.rfind('#') {
            None => (Some(href), None),
            Some(p) if p == 0 => (None, Some(&href[1..])),
            Some(p) => (Some(&href[..p]), Some(&href[(p + 1)..])),
        };

        match (url, id) {
            (None, Some(id)) if !id.is_empty() => Ok(NodeId::Internal(String::from(id))),
            (Some(url), Some(id)) if !id.is_empty() => {
                Ok(NodeId::External(String::from(url), String::from(id)))
            }
            _ => Err(NodeIdError::NodeIdRequired),
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeId::Internal(id) => write!(f, "#{id}"),
            NodeId::External(url, id) => write!(f, "{url}#{id}"),
        }
    }
}

/// A loaded SVG file and its derived data.
pub struct Document {
    /// Tree of nodes; the root is guaranteed to be an `<svg>` element.
    tree: Node,

    /// Metadata about the SVG handle.
    session: Session,

    /// Mapping from `id` attributes to nodes.
    ids: HashMap<String, Node>,

    // The following two require interior mutability because we load the extern
    // resources all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    /// SVG documents referenced from this document.
    externs: RefCell<Resources>,

    /// Image resources referenced from this document.
    images: RefCell<Images>,

    /// Used to load referenced resources.
    load_options: Arc<LoadOptions>,

    /// Stylesheets defined in the document.
    stylesheets: Vec<Stylesheet>,
}

impl Document {
    /// Constructs a `Document` by loading it from a stream.
    pub fn load_from_stream(
        session: Session,
        load_options: Arc<LoadOptions>,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Document, LoadingError> {
        xml_load_from_possibly_compressed_stream(
            session.clone(),
            DocumentBuilder::new(session, load_options.clone()),
            load_options,
            stream,
            cancellable,
        )
    }

    /// Utility function to load a document from a static string in tests.
    #[cfg(test)]
    pub fn load_from_bytes(input: &'static [u8]) -> Document {
        use glib::prelude::*;

        let bytes = glib::Bytes::from_static(input);
        let stream = gio::MemoryInputStream::from_bytes(&bytes);

        Document::load_from_stream(
            Session::new_for_test_suite(),
            Arc::new(LoadOptions::new(UrlResolver::new(None))),
            &stream.upcast(),
            None::<&gio::Cancellable>,
        )
        .unwrap()
    }

    /// Gets the root node.  This is guaranteed to be an `<svg>` element.
    pub fn root(&self) -> Node {
        self.tree.clone()
    }

    /// Looks up a node in this document or one of its resources by its `id` attribute.
    pub fn lookup_node(&self, node_id: &NodeId) -> Option<Node> {
        match node_id {
            NodeId::Internal(id) => self.lookup_internal_node(id),
            NodeId::External(url, id) => self
                .externs
                .borrow_mut()
                .lookup(&self.session, &self.load_options, url, id)
                .ok(),
        }
    }

    /// Looks up a node in this document by its `id` attribute.
    pub fn lookup_internal_node(&self, id: &str) -> Option<Node> {
        self.ids.get(id).map(|n| (*n).clone())
    }

    /// Loads an image by URL, or returns a pre-loaded one.
    pub fn lookup_image(&self, url: &str) -> Result<SharedImageSurface, LoadingError> {
        let aurl = self
            .load_options
            .url_resolver
            .resolve_href(url)
            .map_err(|_| LoadingError::BadUrl)?;

        self.images.borrow_mut().lookup(&self.load_options, &aurl)
    }

    /// Runs the CSS cascade on the document tree
    ///
    /// This uses the default UserAgent stylesheet, the document's internal stylesheets,
    /// plus an extra set of stylesheets supplied by the caller.
    pub fn cascade(&mut self, extra: &[Stylesheet], session: &Session) {
        css::cascade(
            &mut self.tree,
            &UA_STYLESHEETS,
            &self.stylesheets,
            extra,
            session,
        );
    }
}

struct Resources {
    resources: HashMap<AllowedUrl, Result<Rc<Document>, LoadingError>>,
}

impl Resources {
    pub fn new() -> Resources {
        Resources {
            resources: Default::default(),
        }
    }

    pub fn lookup(
        &mut self,
        session: &Session,
        load_options: &LoadOptions,
        url: &str,
        id: &str,
    ) -> Result<Node, LoadingError> {
        self.get_extern_document(session, load_options, url)
            .and_then(|doc| doc.lookup_internal_node(id).ok_or(LoadingError::BadUrl))
    }

    fn get_extern_document(
        &mut self,
        session: &Session,
        load_options: &LoadOptions,
        href: &str,
    ) -> Result<Rc<Document>, LoadingError> {
        let aurl = load_options
            .url_resolver
            .resolve_href(href)
            .map_err(|_| LoadingError::BadUrl)?;

        match self.resources.entry(aurl) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let aurl = e.key();
                // FIXME: pass a cancellable to these
                let doc = io::acquire_stream(aurl, None)
                    .map_err(LoadingError::from)
                    .and_then(|stream| {
                        Document::load_from_stream(
                            session.clone(),
                            Arc::new(load_options.copy_with_base_url(aurl)),
                            &stream,
                            None,
                        )
                    })
                    .map(Rc::new);
                let res = e.insert(doc);
                res.clone()
            }
        }
    }
}

struct Images {
    images: HashMap<AllowedUrl, Result<SharedImageSurface, LoadingError>>,
}

impl Images {
    fn new() -> Images {
        Images {
            images: Default::default(),
        }
    }

    fn lookup(
        &mut self,
        load_options: &LoadOptions,
        aurl: &AllowedUrl,
    ) -> Result<SharedImageSurface, LoadingError> {
        match self.images.entry(aurl.clone()) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let surface = load_image(load_options, e.key());
                let res = e.insert(surface);
                res.clone()
            }
        }
    }
}

fn load_image(
    load_options: &LoadOptions,
    aurl: &AllowedUrl,
) -> Result<SharedImageSurface, LoadingError> {
    let BinaryData {
        data: bytes,
        mime_type,
    } = io::acquire_data(aurl, None)?;

    if bytes.is_empty() {
        return Err(LoadingError::Other(String::from("no image data")));
    }

    let content_type = content_type_for_gdk_pixbuf(&mime_type);

    let loader = if let Some(ref content_type) = content_type {
        PixbufLoader::with_mime_type(content_type)?
    } else {
        PixbufLoader::new()
    };

    loader.write(&bytes)?;
    loader.close()?;

    let pixbuf = loader.pixbuf().ok_or_else(|| {
        LoadingError::Other(format!("loading image: {}", human_readable_url(aurl)))
    })?;

    let bytes = if load_options.keep_image_data {
        Some(bytes)
    } else {
        None
    };

    let surface = SharedImageSurface::from_pixbuf(&pixbuf, content_type.as_deref(), bytes)
        .map_err(|e| image_loading_error_from_cairo(e, aurl))?;

    Ok(surface)
}

fn content_type_for_gdk_pixbuf(mime_type: &Mime) -> Option<String> {
    // See issue #548 - data: URLs without a MIME-type automatically
    // fall back to "text/plain;charset=US-ASCII".  Some (old?) versions of
    // Adobe Illustrator generate data: URLs without MIME-type for image
    // data.  We'll catch this and fall back to sniffing by unsetting the
    // content_type.
    let unspecified_mime_type = Mime::from_str("text/plain;charset=US-ASCII").unwrap();

    if *mime_type == unspecified_mime_type {
        None
    } else {
        Some(format!("{}/{}", mime_type.type_, mime_type.subtype))
    }
}

fn human_readable_url(aurl: &AllowedUrl) -> &str {
    if aurl.scheme() == "data" {
        // avoid printing a huge data: URL for image data
        "data URL"
    } else {
        aurl.as_ref()
    }
}

fn image_loading_error_from_cairo(status: cairo::Error, aurl: &AllowedUrl) -> LoadingError {
    let url = human_readable_url(aurl);

    match status {
        cairo::Error::NoMemory => LoadingError::OutOfMemory(format!("loading image: {url}")),
        cairo::Error::InvalidSize => LoadingError::Other(format!("image too big: {url}")),
        _ => LoadingError::Other(format!("cairo error: {status}")),
    }
}

pub struct AcquiredNode {
    stack: Option<Rc<RefCell<NodeStack>>>,
    node: Node,
}

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        if let Some(ref stack) = self.stack {
            let mut stack = stack.borrow_mut();
            let last = stack.pop().unwrap();
            assert!(last == self.node);
        }
    }
}

impl AcquiredNode {
    pub fn get(&self) -> &Node {
        &self.node
    }
}

/// Detects circular references between nodes, and enforces referencing limits.
///
/// Consider this fragment of SVG:
///
/// ```xml
/// <pattern id="foo">
///   <rect width="1" height="1" fill="url(#foo)"/>
/// </pattern>
/// ```
///
/// The pattern has a child element that references the pattern itself.  This kind of circular
/// reference is invalid.  The `AcquiredNodes` struct is passed around
/// wherever it may be necessary to resolve references to nodes, or to access nodes
/// "elsewhere" in the DOM that is not the current subtree.
///
/// Also, such constructs that reference other elements can be maliciously arranged like
/// in the [billion laughs attack][lol], to cause huge amounts of CPU to be consumed through
/// creating an exponential number of references.  `AcquiredNodes` imposes a hard limit on
/// the number of references that can be resolved for typical, well-behaved SVG documents.
///
/// The [`Self::acquire()`] and [`Self::acquire_ref()`] methods return an [`AcquiredNode`], which
/// acts like a smart pointer for a [`Node`].  Once a node has been acquired, it cannot be
/// acquired again until its [`AcquiredNode`] is dropped.  In the example above, a graphic element
/// would acquire the `pattern`, which would then acquire its `rect` child, which then would fail
/// to re-acquired the `pattern` — thus signaling a circular reference.
///
/// Those methods return an [`AcquireError`] to signal circular references.  Also, they
/// can return [`AcquireError::MaxReferencesExceeded`] if the aforementioned referencing limit
/// is exceeded.
///
/// [lol]: https://bitbucket.org/tiran/defusedxml
pub struct AcquiredNodes<'i> {
    document: &'i Document,
    num_elements_acquired: usize,
    node_stack: Rc<RefCell<NodeStack>>,
}

impl<'i> AcquiredNodes<'i> {
    pub fn new(document: &Document) -> AcquiredNodes<'_> {
        AcquiredNodes {
            document,
            num_elements_acquired: 0,
            node_stack: Rc::new(RefCell::new(NodeStack::new())),
        }
    }

    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, LoadingError> {
        self.document.lookup_image(href)
    }

    /// Acquires a node by its id.
    ///
    /// This is typically used during an "early resolution" stage, when XML `id`s are being
    /// resolved to node references.
    pub fn acquire(&mut self, node_id: &NodeId) -> Result<AcquiredNode, AcquireError> {
        self.num_elements_acquired += 1;

        // This is a mitigation for SVG files that try to instance a huge number of
        // elements via <use>, recursive patterns, etc.  See limits.rs for details.
        if self.num_elements_acquired > limits::MAX_REFERENCED_ELEMENTS {
            return Err(AcquireError::MaxReferencesExceeded);
        }

        // FIXME: callers shouldn't have to know that get_node() can initiate a file load.
        // Maybe we should have the following stages:
        //   - load main SVG XML
        //
        //   - load secondary SVG XML and other files like images; all document::Resources and
        //     document::Images loaded
        //
        //   - Now that all files are loaded, resolve URL references
        let node = self
            .document
            .lookup_node(node_id)
            .ok_or_else(|| AcquireError::LinkNotFound(node_id.clone()))?;

        if !node.is_element() {
            return Err(AcquireError::InvalidLinkType(node_id.clone()));
        }

        if node.borrow_element().is_accessed_by_reference() {
            self.acquire_ref(&node)
        } else {
            Ok(AcquiredNode { stack: None, node })
        }
    }

    /// Acquires a node whose reference is already known.
    ///
    /// This is useful for cases where a node is initially referenced by its id with
    /// [`Self::acquire`] and kept around for later use.  During the later use, the node
    /// needs to be re-acquired with this method.  For example:
    ///
    /// * At an "early resolution" stage, `acquire()` a pattern by its id, and keep around its
    /// [`Node`] reference.
    ///
    /// * At the drawing stage, `acquire_ref()` the pattern node that we already had, so that
    /// its child elements that reference other paint servers will be able to detect circular
    /// references to the pattern.
    pub fn acquire_ref(&self, node: &Node) -> Result<AcquiredNode, AcquireError> {
        if self.node_stack.borrow().contains(node) {
            Err(AcquireError::CircularReference(node.clone()))
        } else {
            self.node_stack.borrow_mut().push(node);
            Ok(AcquiredNode {
                stack: Some(self.node_stack.clone()),
                node: node.clone(),
            })
        }
    }
}

/// Keeps a stack of nodes and can check if a certain node is contained in the stack
///
/// Sometimes parts of the code cannot plainly use the implicit stack of acquired
/// nodes as maintained by DrawingCtx::acquire_node(), and they must keep their
/// own stack of nodes to test for reference cycles.  NodeStack can be used to do that.
pub struct NodeStack(Vec<Node>);

impl NodeStack {
    pub fn new() -> NodeStack {
        NodeStack(Vec::new())
    }

    pub fn push(&mut self, node: &Node) {
        self.0.push(node.clone());
    }

    pub fn pop(&mut self) -> Option<Node> {
        self.0.pop()
    }

    pub fn contains(&self, node: &Node) -> bool {
        self.0.iter().any(|n| *n == *node)
    }
}

/// Used to build a tree of SVG nodes while an XML document is being read.
///
/// This struct holds the document-related state while loading an SVG document from XML:
/// the loading options, the partially-built tree of nodes, the CSS stylesheets that
/// appear while loading the document.
///
/// The XML loader asks a `DocumentBuilder` to
/// [`append_element`][DocumentBuilder::append_element],
/// [`append_characters`][DocumentBuilder::append_characters], etc.  When all the XML has
/// been consumed, the caller can use [`build`][DocumentBuilder::build] to get a
/// fully-loaded [`Document`].
pub struct DocumentBuilder {
    /// Metadata for the document's lifetime.
    session: Session,

    /// Loading options; mainly the URL resolver.
    load_options: Arc<LoadOptions>,

    /// Root node of the tree.
    tree: Option<Node>,

    /// Mapping from `id` attributes to nodes.
    ids: HashMap<String, Node>,

    /// Stylesheets defined in the document.
    stylesheets: Vec<Stylesheet>,
}

impl DocumentBuilder {
    pub fn new(session: Session, load_options: Arc<LoadOptions>) -> DocumentBuilder {
        DocumentBuilder {
            session,
            load_options,
            tree: None,
            ids: HashMap::new(),
            stylesheets: Vec::new(),
        }
    }

    /// Adds a stylesheet in order to the document.
    ///
    /// Stylesheets will later be matched in the order in which they were added.
    pub fn append_stylesheet(&mut self, stylesheet: Stylesheet) {
        self.stylesheets.push(stylesheet);
    }

    /// Creates an element of the specified `name` as a child of `parent`.
    ///
    /// This is the main function to create new SVG elements while parsing XML.
    ///
    /// `name` is the XML element's name, for example `rect`.
    ///
    /// `attrs` has the XML element's attributes, e.g. cx/cy/r for `<circle cx="0" cy="0"
    /// r="5">`.
    ///
    /// If `parent` is `None` it means that we are creating the root node in the tree of
    /// elements.  The code will later validate that this is indeed an `<svg>` element.
    pub fn append_element(
        &mut self,
        name: &QualName,
        attrs: Attributes,
        parent: Option<Node>,
    ) -> Node {
        let node = Node::new(NodeData::new_element(&self.session, name, attrs));

        if let Some(id) = node.borrow_element().get_id() {
            // This is so we don't overwrite an existing id
            self.ids
                .entry(id.to_string())
                .or_insert_with(|| node.clone());
        }

        if let Some(parent) = parent {
            parent.append(node.clone());
        } else if self.tree.is_none() {
            self.tree = Some(node.clone());
        } else {
            panic!("The tree root has already been set");
        }

        node
    }

    /// Creates a node for an XML text element as a child of `parent`.
    pub fn append_characters(&mut self, text: &str, parent: &mut Node) {
        if !text.is_empty() {
            // When the last child is a Chars node we can coalesce
            // the text and avoid screwing up the Pango layouts
            if let Some(child) = parent.last_child().filter(|c| c.is_chars()) {
                child.borrow_chars().append(text);
            } else {
                parent.append(Node::new(NodeData::new_chars(text)));
            };
        }
    }

    /// Does the final validation on the `Document` being read, and returns it.
    pub fn build(self) -> Result<Document, LoadingError> {
        let DocumentBuilder {
            load_options,
            session,
            tree,
            ids,
            stylesheets,
            ..
        } = self;

        match tree {
            Some(root) if root.is_element() => {
                if is_element_of_type!(root, Svg) {
                    let mut document = Document {
                        tree: root,
                        session: session.clone(),
                        ids,
                        externs: RefCell::new(Resources::new()),
                        images: RefCell::new(Images::new()),
                        load_options,
                        stylesheets,
                    };

                    document.cascade(&[], &session);

                    Ok(document)
                } else {
                    Err(LoadingError::NoSvgRoot)
                }
            }
            _ => Err(LoadingError::NoSvgRoot),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_id() {
        assert_eq!(
            NodeId::parse("#foo").unwrap(),
            NodeId::Internal("foo".to_string())
        );

        assert_eq!(
            NodeId::parse("uri#foo").unwrap(),
            NodeId::External("uri".to_string(), "foo".to_string())
        );

        assert!(matches!(
            NodeId::parse("uri"),
            Err(NodeIdError::NodeIdRequired)
        ));
    }

    #[test]
    fn unspecified_mime_type_yields_no_content_type() {
        // Issue #548
        let mime = Mime::from_str("text/plain;charset=US-ASCII").unwrap();
        assert!(content_type_for_gdk_pixbuf(&mime).is_none());
    }

    #[test]
    fn strips_mime_type_parameters() {
        // Issue #699
        let mime = Mime::from_str("image/png;charset=utf-8").unwrap();
        assert_eq!(
            content_type_for_gdk_pixbuf(&mime),
            Some(String::from("image/png"))
        );
    }
}
