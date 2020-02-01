//! Main SVG document structure.

use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use markup5ever::{LocalName, Namespace, QualName};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use crate::allowed_url::{AllowedUrl, AllowedUrlError, Fragment};
use crate::create_node::create_node;
use crate::css::{self, Origin, Stylesheet};
use crate::error::{AcquireError, LoadingError};
use crate::handle::LoadOptions;
use crate::io::{self, BinaryData};
use crate::limits;
use crate::node::{NodeData, NodeType, RsvgNode};
use crate::property_bag::PropertyBag;
use crate::structure::{IntrinsicDimensions, Svg};
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::text::NodeChars;
use crate::xml::xml_load_from_possibly_compressed_stream;

/// A loaded SVG file and its derived data.
pub struct Document {
    /// Tree of nodes; the root is guaranteed to be an `<svg>` element.
    tree: RsvgNode,

    /// Mapping from `id` attributes to nodes.
    ids: HashMap<String, RsvgNode>,

    // The following two require interior mutability because we load the extern
    // resources all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    /// SVG documents referenced from this document.
    externs: RefCell<Resources>,

    /// Image resources referenced from this document.
    images: RefCell<Images>,

    /// Used to load referenced resources.
    load_options: LoadOptions,

    /// Stylesheets defined in the document
    stylesheets: Vec<Stylesheet>,
}

impl Document {
    /// Constructs a `Document` by loading it from a stream.
    pub fn load_from_stream(
        load_options: &LoadOptions,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Document, LoadingError> {
        xml_load_from_possibly_compressed_stream(
            DocumentBuilder::new(load_options),
            load_options.unlimited_size,
            stream,
            cancellable,
        )
    }

    /// Gets the root node.  This is guaranteed to be an `<svg>` element.
    pub fn root(&self) -> RsvgNode {
        self.tree.clone()
    }

    /// Looks up an element node by its URL.
    ///
    /// This is also used to find elements in referenced resources, as in
    /// `xlink:href="subresource.svg#element_name".
    pub fn lookup(&self, fragment: &Fragment) -> Result<RsvgNode, LoadingError> {
        if fragment.uri().is_some() {
            self.externs
                .borrow_mut()
                .lookup(&self.load_options, fragment)
        } else {
            self.lookup_node_by_id(fragment.fragment())
                .ok_or(LoadingError::BadUrl)
        }
    }

    /// Looks up a node only in this document fragment by its `id` attribute.
    pub fn lookup_node_by_id(&self, id: &str) -> Option<RsvgNode> {
        self.ids.get(id).map(|n| (*n).clone())
    }

    /// Loads an image by URL, or returns a pre-loaded one.
    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, LoadingError> {
        let aurl = AllowedUrl::from_href(href, self.load_options.base_url.as_ref())
            .map_err(|_| LoadingError::BadUrl)?;

        self.images.borrow_mut().lookup(&self.load_options, &aurl)
    }

    /// Gets the dimension parameters of the toplevel `<svg>`.
    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let root = self.root();
        let node_data = root.borrow();

        assert!(node_data.get_type() == NodeType::Svg);
        node_data.get_impl::<Svg>().get_intrinsic_dimensions()
    }

    /// Runs the CSS cascade on the document tree
    ///
    /// This uses the document's internal stylesheets, plus an extra set of stylesheets
    /// supplied by the caller.
    pub fn cascade(&mut self, extra: &[Stylesheet]) {
        css::cascade(&mut self.tree, &self.stylesheets, extra);
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
        load_options: &LoadOptions,
        fragment: &Fragment,
    ) -> Result<RsvgNode, LoadingError> {
        if let Some(ref href) = fragment.uri() {
            self.get_extern_document(load_options, href)
                .and_then(|doc| {
                    doc.lookup_node_by_id(fragment.fragment())
                        .ok_or(LoadingError::BadUrl)
                })
        } else {
            unreachable!();
        }
    }

    fn get_extern_document(
        &mut self,
        load_options: &LoadOptions,
        href: &str,
    ) -> Result<Rc<Document>, LoadingError> {
        let aurl = AllowedUrl::from_href(href, load_options.base_url.as_ref())
            .map_err(|_| LoadingError::BadUrl)?;

        match self.resources.entry(aurl) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let aurl = e.key();
                // FIXME: pass a cancellable to these
                let doc = io::acquire_stream(aurl, None)
                    .and_then(|stream| {
                        Document::load_from_stream(
                            &load_options.copy_with_base_url(aurl),
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
        content_type,
    } = io::acquire_data(&aurl, None)?;

    if bytes.is_empty() {
        return Err(LoadingError::EmptyData);
    }

    let loader = if let Some(ref content_type) = content_type {
        PixbufLoader::new_with_mime_type(content_type)?
    } else {
        PixbufLoader::new()
    };

    loader.write(&bytes)?;
    loader.close()?;

    let pixbuf = loader.get_pixbuf().ok_or(LoadingError::Unknown)?;

    let bytes = if load_options.keep_image_data {
        Some(bytes)
    } else {
        None
    };

    let surface =
        SharedImageSurface::from_pixbuf(&pixbuf, bytes, content_type.as_ref().map(String::as_str))?;

    Ok(surface)
}

pub struct AcquiredNode {
    stack: Option<Rc<RefCell<NodeStack>>>,
    node: RsvgNode,
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
    pub fn get(&self) -> &RsvgNode {
        &self.node
    }
}

/// This helper struct is used when looking up urls to other nodes.
/// Its methods do recursion checking and thereby avoid infinite loops.
///
/// Malformed SVGs, for example, may reference a marker by its IRI, but
/// the object referenced by the IRI is not a marker.
///
/// Note that if you acquire a node, you have to release it before trying to
/// acquire it again.  If you acquire a node "#foo" and don't release it before
/// trying to acquire "foo" again, you will obtain a None the second time.
pub struct AcquiredNodes<'i> {
    document: &'i Document,
    num_elements_acquired: usize,
    node_stack: Rc<RefCell<NodeStack>>,
}

impl<'i> AcquiredNodes<'i> {
    pub fn new(document: &Document) -> AcquiredNodes {
        AcquiredNodes {
            document,
            num_elements_acquired: 0,
            node_stack: Rc::new(RefCell::new(NodeStack::new())),
        }
    }

    pub fn lookup_node(
        &self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<RsvgNode, AcquireError> {
        let node = self.document.lookup(fragment).map_err(|_| {
            // FIXME: callers shouldn't have to know that get_node() can initiate a file load.
            // Maybe we should have the following stages:
            //   - load main SVG XML
            //
            //   - load secondary SVG XML and other files like images; all document::Resources and
            //     document::Images loaded
            //
            //   - Now that all files are loaded, resolve URL references
            AcquireError::LinkNotFound(fragment.clone())
        })?;

        if node_types.is_empty() {
            Ok(node)
        } else {
            let node_type = node.borrow().get_type();
            if node_types.iter().find(|&&t| t == node_type).is_some() {
                Ok(node)
            } else {
                Err(AcquireError::InvalidLinkType(fragment.clone()))
            }
        }
    }

    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, LoadingError> {
        self.document.lookup_image(href)
    }

    /// Acquires a node.
    /// Specify `node_types` when expecting the node to be of a particular type,
    /// or use an empty slice for `node_types` if you want a node of any type.
    /// Nodes acquired by this function must be released in reverse acquiring order.
    pub fn acquire(
        &mut self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<AcquiredNode, AcquireError> {
        self.num_elements_acquired += 1;

        // This is a mitigation for SVG files that try to instance a huge number of
        // elements via <use>, recursive patterns, etc.  See limits.rs for details.
        if self.num_elements_acquired > limits::MAX_REFERENCED_ELEMENTS {
            return Err(AcquireError::MaxReferencesExceeded);
        }

        let node = self.lookup_node(fragment, node_types)?;

        if node_is_accessed_by_reference(&node) {
            self.acquire_ref(&node)
        } else {
            Ok(AcquiredNode {
                stack: None,
                node: node.clone(),
            })
        }
    }

    pub fn acquire_ref(&self, node: &RsvgNode) -> Result<AcquiredNode, AcquireError> {
        if self.node_stack.borrow().contains(&node) {
            Err(AcquireError::CircularReference(node.clone()))
        } else {
            self.node_stack.borrow_mut().push(&node);
            Ok(AcquiredNode {
                stack: Some(self.node_stack.clone()),
                node: node.clone(),
            })
        }
    }
}

// Returns whether a node of a particular type is only accessed by reference
// from other nodes' atributes.  The node could in turn cause other nodes
// to get referenced, potentially causing reference cycles.
fn node_is_accessed_by_reference(node: &RsvgNode) -> bool {
    use NodeType::*;

    match node.borrow().get_type() {
        ClipPath | Filter | LinearGradient | Marker | Mask | Pattern | RadialGradient => true,

        _ => false,
    }
}

/// Keeps a stack of nodes and can check if a certain node is contained in the stack
///
/// Sometimes parts of the code cannot plainly use the implicit stack of acquired
/// nodes as maintained by DrawingCtx::acquire_node(), and they must keep their
/// own stack of nodes to test for reference cycles.  NodeStack can be used to do that.
pub struct NodeStack(Vec<RsvgNode>);

impl NodeStack {
    pub fn new() -> NodeStack {
        NodeStack(Vec::new())
    }

    pub fn push(&mut self, node: &RsvgNode) {
        self.0.push(node.clone());
    }

    pub fn pop(&mut self) -> Option<RsvgNode> {
        self.0.pop()
    }

    pub fn contains(&self, node: &RsvgNode) -> bool {
        self.0.iter().find(|n| **n == *node).is_some()
    }
}

pub struct DocumentBuilder {
    load_options: LoadOptions,
    tree: Option<RsvgNode>,
    ids: HashMap<String, RsvgNode>,
    stylesheets: Vec<Stylesheet>,
}

impl DocumentBuilder {
    pub fn new(load_options: &LoadOptions) -> DocumentBuilder {
        DocumentBuilder {
            load_options: load_options.clone(),
            tree: None,
            ids: HashMap::new(),
            stylesheets: Vec::new(),
        }
    }

    pub fn append_stylesheet_from_xml_processing_instruction(
        &mut self,
        alternate: Option<String>,
        type_: Option<String>,
        href: &str,
    ) -> Result<(), LoadingError> {
        if type_.as_ref().map(String::as_str) != Some("text/css")
            || (alternate.is_some() && alternate.as_ref().map(String::as_str) != Some("no"))
        {
            return Err(LoadingError::BadStylesheet);
        }

        // FIXME: handle CSS errors
        if let Ok(stylesheet) =
            Stylesheet::from_href(href, self.load_options.base_url.as_ref(), Origin::Author)
        {
            self.stylesheets.push(stylesheet);
        }

        Ok(())
    }

    pub fn append_element(
        &mut self,
        name: &QualName,
        pbag: &PropertyBag,
        parent: Option<RsvgNode>,
    ) -> RsvgNode {
        let mut node = create_node(name, pbag);

        if let Some(id) = node.borrow().get_id() {
            // This is so we don't overwrite an existing id
            self.ids
                .entry(id.to_string())
                .or_insert_with(|| node.clone());
        }

        node.borrow_mut()
            .set_atts(parent.as_ref().clone(), pbag, self.load_options.locale());

        if let Some(mut parent) = parent {
            parent.append(node.clone());
        } else if self.tree.is_none() {
            self.tree = Some(node.clone());
        } else {
            panic!("The tree root has already been set");
        }

        node
    }

    pub fn append_stylesheet_from_text(&mut self, text: &str) {
        // FIXME: handle CSS errors
        if let Ok(stylesheet) =
            Stylesheet::from_data(text, self.load_options.base_url.as_ref(), Origin::Author)
        {
            self.stylesheets.push(stylesheet);
        }
    }

    pub fn append_characters(&mut self, text: &str, parent: &mut RsvgNode) {
        if !text.is_empty() {
            self.append_chars_to_parent(text, parent);
        }
    }

    fn append_chars_to_parent(&mut self, text: &str, parent: &mut RsvgNode) {
        // When the last child is a Chars node we can coalesce
        // the text and avoid screwing up the Pango layouts
        let chars_node = if let Some(child) = parent
            .last_child()
            .filter(|c| c.borrow().get_type() == NodeType::Chars)
        {
            child
        } else {
            let child = RsvgNode::new(NodeData::new(
                NodeType::Chars,
                &QualName::new(
                    None,
                    Namespace::from("https://wiki.gnome.org/Projects/LibRsvg"),
                    LocalName::from("rsvg-chars"),
                ),
                None,
                None,
                Box::new(NodeChars::new()),
            ));

            parent.append(child.clone());

            child
        };

        chars_node.borrow().get_impl::<NodeChars>().append(text);
    }

    pub fn resolve_href(&self, href: &str) -> Result<AllowedUrl, AllowedUrlError> {
        AllowedUrl::from_href(href, self.load_options.base_url.as_ref())
    }

    pub fn build(self) -> Result<Document, LoadingError> {
        let DocumentBuilder {
            load_options,
            tree,
            ids,
            stylesheets,
            ..
        } = self;

        match tree {
            None => Err(LoadingError::SvgHasNoElements),
            Some(root) => {
                if root.borrow().get_type() == NodeType::Svg {
                    let mut document = Document {
                        tree: root.clone(),
                        ids,
                        externs: RefCell::new(Resources::new()),
                        images: RefCell::new(Images::new()),
                        load_options: load_options.clone(),
                        stylesheets,
                    };

                    document.cascade(&[]);

                    Ok(document)
                } else {
                    Err(LoadingError::RootElementIsNotSvg)
                }
            }
        }
    }
}
