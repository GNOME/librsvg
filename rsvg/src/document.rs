//! Main SVG document structure.

use data_url::mime::Mime;
use glib::prelude::*;
use markup5ever::QualName;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::include_str;
use std::io::Cursor;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::{cell::RefCell, sync::OnceLock};

use crate::accept_language::UserLanguage;
use crate::bbox::BoundingBox;
use crate::borrow_element_as;
use crate::css::{self, Origin, Stylesheet};
use crate::dpi::Dpi;
use crate::drawing_ctx::{draw_tree, with_saved_cr, DrawingMode, SvgNesting};
use crate::error::{AcquireError, InternalRenderingError, LoadingError, NodeIdError};
use crate::io::{self, BinaryData};
use crate::is_element_of_type;
use crate::limits;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeData};
use crate::rect::Rect;
use crate::session::Session;
use crate::structure::IntrinsicDimensions;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::url_resolver::{AllowedUrl, UrlResolver};
use crate::xml::{xml_load_from_possibly_compressed_stream, Attributes};

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
            Some(0) => (None, Some(&href[1..])),
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

/// Loading options for SVG documents.
pub struct LoadOptions {
    /// Load url resolver; all references will be resolved with respect to this.
    pub url_resolver: UrlResolver,

    /// Whether to turn off size limits in libxml2.
    pub unlimited_size: bool,

    /// Whether to keep original (undecoded) image data to embed in Cairo PDF surfaces.
    pub keep_image_data: bool,
}

impl LoadOptions {
    /// Creates a `LoadOptions` with defaults, and sets the `url resolver`.
    pub fn new(url_resolver: UrlResolver) -> Self {
        LoadOptions {
            url_resolver,
            unlimited_size: false,
            keep_image_data: false,
        }
    }

    /// Sets whether libxml2's limits on memory usage should be turned off.
    ///
    /// This should only be done for trusted data.
    pub fn with_unlimited_size(mut self, unlimited: bool) -> Self {
        self.unlimited_size = unlimited;
        self
    }

    /// Sets whether to keep the original compressed image data from referenced JPEG/PNG images.
    ///
    /// This is only useful for rendering to Cairo PDF
    /// surfaces, which can embed the original, compressed image data instead of uncompressed
    /// RGB buffers.
    pub fn keep_image_data(mut self, keep: bool) -> Self {
        self.keep_image_data = keep;
        self
    }

    /// Creates a new `LoadOptions` with a different `url resolver`.
    ///
    /// This is used when loading a referenced file that may in turn cause other files
    /// to be loaded, for example `<image xlink:href="subimage.svg"/>`
    pub fn copy_with_base_url(&self, base_url: &AllowedUrl) -> Self {
        let mut url_resolver = self.url_resolver.clone();
        url_resolver.base_url = Some((**base_url).clone());

        LoadOptions {
            url_resolver,
            unlimited_size: self.unlimited_size,
            keep_image_data: self.keep_image_data,
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

    /// Othewr SVG documents and images referenced from this document.
    ///
    /// This requires requires interior mutability because we load resources all over the
    /// place.  Eventually we'll be able to do this once, at loading time, and keep this
    /// immutable.
    resources: RefCell<Resources>,

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
    fn lookup_node(&self, node_id: &NodeId) -> Option<Node> {
        match node_id {
            NodeId::Internal(id) => self.lookup_internal_node(id),
            NodeId::External(url, id) => self
                .resources
                .borrow_mut()
                .lookup_node(&self.session, &self.load_options, url, id)
                .ok(),
        }
    }

    /// Looks up a node in this document by its `id` attribute.
    pub fn lookup_internal_node(&self, id: &str) -> Option<Node> {
        self.ids.get(id).map(|n| (*n).clone())
    }

    /// Loads an image by URL, or returns a pre-loaded one.
    fn lookup_image(&self, url: &str) -> Result<SharedImageSurface, LoadingError> {
        let aurl = self
            .load_options
            .url_resolver
            .resolve_href(url)
            .map_err(|_| LoadingError::BadUrl)?;

        self.resources
            .borrow_mut()
            .lookup_image(&self.session, &self.load_options, &aurl)
    }

    /// Loads a resource by URL, or returns a pre-loaded one.
    fn lookup_resource(&self, url: &str) -> Result<Resource, LoadingError> {
        let aurl = self
            .load_options
            .url_resolver
            .resolve_href(url)
            .map_err(|_| LoadingError::BadUrl)?;

        // FIXME: pass a cancellable to this.  This function is called
        // at rendering time, so probably the cancellable should come
        // from cancellability in CairoRenderer - see #429
        self.resources
            .borrow_mut()
            .lookup_resource(&self.session, &self.load_options, &aurl, None)
    }

    /// Runs the CSS cascade on the document tree
    ///
    /// This uses the default UserAgent stylesheet, the document's internal stylesheets,
    /// plus an extra set of stylesheets supplied by the caller.
    pub fn cascade(&mut self, extra: &[Stylesheet], session: &Session) {
        let stylesheets = {
            static UA_STYLESHEETS: OnceLock<Vec<Stylesheet>> = OnceLock::new();
            UA_STYLESHEETS.get_or_init(|| {
                vec![Stylesheet::from_data(
                    include_str!("ua.css"),
                    &UrlResolver::new(None),
                    Origin::UserAgent,
                    Session::default(),
                )
                .expect("could not parse user agent stylesheet for librsvg, there's a bug!")]
            })
        };
        css::cascade(
            &mut self.tree,
            stylesheets,
            &self.stylesheets,
            extra,
            session,
        );
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let root = self.root();
        let cascaded = CascadedValues::new_from_node(&root);
        let values = cascaded.get();
        borrow_element_as!(self.root(), Svg).get_intrinsic_dimensions(values)
    }

    pub fn render_document(
        &self,
        session: &Session,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        svg_nesting: SvgNesting,
        is_testing: bool,
    ) -> Result<(), InternalRenderingError> {
        let root = self.root();
        self.render_layer(
            session,
            cr,
            root,
            viewport,
            user_language,
            dpi,
            svg_nesting,
            is_testing,
        )
    }

    pub fn render_layer(
        &self,
        session: &Session,
        cr: &cairo::Context,
        node: Node,
        viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        svg_nesting: SvgNesting,
        is_testing: bool,
    ) -> Result<(), InternalRenderingError> {
        cr.status()?;

        let root = self.root();

        let viewport = Rect::from(*viewport);

        with_saved_cr(cr, || {
            draw_tree(
                session.clone(),
                DrawingMode::LimitToStack { node, root },
                cr,
                viewport,
                user_language,
                dpi,
                svg_nesting,
                false,
                is_testing,
                &mut AcquiredNodes::new(self),
            )
            .map(|_bbox| ())
        })
    }

    fn geometry_for_layer(
        &self,
        session: &Session,
        node: Node,
        viewport: Rect,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(Rect, Rect), InternalRenderingError> {
        let root = self.root();

        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target)?;

        let bbox = draw_tree(
            session.clone(),
            DrawingMode::LimitToStack { node, root },
            &cr,
            viewport,
            user_language,
            dpi,
            SvgNesting::Standalone,
            true,
            is_testing,
            &mut AcquiredNodes::new(self),
        )?;

        let ink_rect = bbox.ink_rect.unwrap_or_default();
        let logical_rect = bbox.rect.unwrap_or_default();

        Ok((ink_rect, logical_rect))
    }

    pub fn get_geometry_for_layer(
        &self,
        session: &Session,
        node: Node,
        viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), InternalRenderingError> {
        let viewport = Rect::from(*viewport);

        let (ink_rect, logical_rect) =
            self.geometry_for_layer(session, node, viewport, user_language, dpi, is_testing)?;

        Ok((
            cairo::Rectangle::from(ink_rect),
            cairo::Rectangle::from(logical_rect),
        ))
    }

    fn get_bbox_for_element(
        &self,
        session: &Session,
        node: &Node,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target)?;

        let node = node.clone();

        draw_tree(
            session.clone(),
            DrawingMode::OnlyNode(node),
            &cr,
            unit_rectangle(),
            user_language,
            dpi,
            SvgNesting::Standalone,
            true,
            is_testing,
            &mut AcquiredNodes::new(self),
        )
    }

    /// Returns (ink_rect, logical_rect)
    pub fn get_geometry_for_element(
        &self,
        session: &Session,
        node: Node,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), InternalRenderingError> {
        let bbox = self.get_bbox_for_element(session, &node, user_language, dpi, is_testing)?;

        let ink_rect = bbox.ink_rect.unwrap_or_default();
        let logical_rect = bbox.rect.unwrap_or_default();

        // Translate so ink_rect is always at offset (0, 0)
        let ofs = (-ink_rect.x0, -ink_rect.y0);

        Ok((
            cairo::Rectangle::from(ink_rect.translate(ofs)),
            cairo::Rectangle::from(logical_rect.translate(ofs)),
        ))
    }

    pub fn render_element(
        &self,
        session: &Session,
        cr: &cairo::Context,
        node: Node,
        element_viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), InternalRenderingError> {
        cr.status()?;

        let bbox = self.get_bbox_for_element(session, &node, user_language, dpi, is_testing)?;

        if bbox.ink_rect.is_none() || bbox.rect.is_none() {
            // Nothing to draw
            return Ok(());
        }

        let ink_r = bbox.ink_rect.unwrap_or_default();

        if ink_r.is_empty() {
            return Ok(());
        }

        // Render, transforming so element is at the new viewport's origin

        with_saved_cr(cr, || {
            let factor = (element_viewport.width() / ink_r.width())
                .min(element_viewport.height() / ink_r.height());

            cr.translate(element_viewport.x(), element_viewport.y());
            cr.scale(factor, factor);
            cr.translate(-ink_r.x0, -ink_r.y0);

            draw_tree(
                session.clone(),
                DrawingMode::OnlyNode(node),
                cr,
                unit_rectangle(),
                user_language,
                dpi,
                SvgNesting::Standalone,
                false,
                is_testing,
                &mut AcquiredNodes::new(self),
            )
            .map(|_bbox| ())
        })
    }
}

fn unit_rectangle() -> Rect {
    Rect::from_size(1.0, 1.0)
}

/// Any kind of resource loaded while processing an SVG document: images, or SVGs themselves.
#[derive(Clone)]
pub enum Resource {
    Document(Rc<Document>),
    Image(SharedImageSurface),
}

struct Resources {
    resources: HashMap<AllowedUrl, Result<Resource, LoadingError>>,
}

impl Resources {
    fn new() -> Resources {
        Resources {
            resources: Default::default(),
        }
    }

    fn lookup_node(
        &mut self,
        session: &Session,
        load_options: &LoadOptions,
        url: &str,
        id: &str,
    ) -> Result<Node, LoadingError> {
        self.get_extern_document(session, load_options, url)
            .and_then(|resource| match resource {
                Resource::Document(doc) => doc.lookup_internal_node(id).ok_or(LoadingError::BadUrl),
                _ => unreachable!("get_extern_document() should already have ensured the document"),
            })
    }

    fn get_extern_document(
        &mut self,
        session: &Session,
        load_options: &LoadOptions,
        href: &str,
    ) -> Result<Resource, LoadingError> {
        let aurl = load_options
            .url_resolver
            .resolve_href(href)
            .map_err(|_| LoadingError::BadUrl)?;

        // FIXME: pass a cancellable to this.  This function is called
        // at rendering time, so probably the cancellable should come
        // from cancellability in CairoRenderer - see #429
        let resource = self.lookup_resource(session, load_options, &aurl, None)?;

        match resource {
            Resource::Document(_) => Ok(resource),
            _ => Err(LoadingError::Other(format!(
                "{href} is not an SVG document"
            ))),
        }
    }

    fn lookup_image(
        &mut self,
        session: &Session,
        load_options: &LoadOptions,
        aurl: &AllowedUrl,
    ) -> Result<SharedImageSurface, LoadingError> {
        // FIXME: pass a cancellable to this.  This function is called
        // at rendering time, so probably the cancellable should come
        // from cancellability in CairoRenderer - see #429
        let resource = self.lookup_resource(session, load_options, aurl, None)?;

        match resource {
            Resource::Image(image) => Ok(image),
            _ => Err(LoadingError::Other(format!("{aurl} is not a raster image"))),
        }
    }

    fn lookup_resource(
        &mut self,
        session: &Session,
        load_options: &LoadOptions,
        aurl: &AllowedUrl,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Resource, LoadingError> {
        match self.resources.entry(aurl.clone()) {
            Entry::Occupied(e) => e.get().clone(),

            Entry::Vacant(e) => {
                let resource_result = load_resource(session, load_options, aurl, cancellable);
                e.insert(resource_result.clone());
                resource_result
            }
        }
    }
}

fn load_resource(
    session: &Session,
    load_options: &LoadOptions,
    aurl: &AllowedUrl,
    cancellable: Option<&gio::Cancellable>,
) -> Result<Resource, LoadingError> {
    let data = io::acquire_data(aurl, cancellable)?;

    let svg_mime_type = Mime::from_str("image/svg+xml").unwrap();

    if data.mime_type == svg_mime_type {
        load_svg_resource_from_bytes(session, load_options, aurl, data, cancellable)
    } else {
        load_image_resource_from_bytes(load_options, aurl, data)
    }
}

fn load_svg_resource_from_bytes(
    session: &Session,
    load_options: &LoadOptions,
    aurl: &AllowedUrl,
    data: BinaryData,
    cancellable: Option<&gio::Cancellable>,
) -> Result<Resource, LoadingError> {
    let BinaryData {
        data: input_bytes,
        mime_type: _mime_type,
    } = data;

    let bytes = glib::Bytes::from_owned(input_bytes);
    let stream = gio::MemoryInputStream::from_bytes(&bytes);

    let document = Document::load_from_stream(
        session.clone(),
        Arc::new(load_options.copy_with_base_url(aurl)),
        &stream.upcast(),
        cancellable,
    )?;

    Ok(Resource::Document(Rc::new(document)))
}

fn load_image_resource_from_bytes(
    load_options: &LoadOptions,
    aurl: &AllowedUrl,
    data: BinaryData,
) -> Result<Resource, LoadingError> {
    let BinaryData {
        data: bytes,
        mime_type,
    } = data;

    if bytes.is_empty() {
        return Err(LoadingError::Other(String::from("no image data")));
    }

    let content_type = content_type_for_image(&mime_type);

    load_image_with_image_rs(aurl, bytes, content_type, load_options)
}

fn image_format(content_type: &str) -> Result<image::ImageFormat, LoadingError> {
    match content_type {
        "image/png" => Ok(image::ImageFormat::Png),
        "image/jpeg" => Ok(image::ImageFormat::Jpeg),
        "image/gif" => Ok(image::ImageFormat::Gif),
        "image/webp" => Ok(image::ImageFormat::WebP),
        _ => Err(LoadingError::Other(format!(
            "unsupported image format {content_type}"
        ))),
    }
}

fn load_image_with_image_rs(
    aurl: &AllowedUrl,
    bytes: Vec<u8>,
    content_type: Option<String>,
    load_options: &LoadOptions,
) -> Result<Resource, LoadingError> {
    let cursor = Cursor::new(&bytes);

    let reader = if let Some(ref content_type) = content_type {
        let format = image_format(content_type)?;
        image::io::Reader::with_format(cursor, format)
    } else {
        image::io::Reader::new(cursor)
            .with_guessed_format()
            .map_err(|_| LoadingError::Other(String::from("unknown image format")))?
    };

    let image = reader
        .decode()
        .map_err(|e| LoadingError::Other(format!("error decoding image: {e}")))?;

    let bytes = if load_options.keep_image_data {
        Some(bytes)
    } else {
        None
    };

    let surface = SharedImageSurface::from_image(&image, content_type.as_deref(), bytes)
        .map_err(|e| image_loading_error_from_cairo(e, aurl))?;

    Ok(Resource::Image(surface))
}

fn content_type_for_image(mime_type: &Mime) -> Option<String> {
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

    pub fn lookup_resource(&self, url: &str) -> Result<Resource, LoadingError> {
        self.document.lookup_resource(url)
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
        //   - load secondary resources: SVG XML and other files like images
        //
        //   - Now that all files are loaded, resolve URL references
        let node = self
            .document
            .lookup_node(node_id)
            .ok_or_else(|| AcquireError::LinkNotFound(node_id.clone()))?;

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
                        resources: RefCell::new(Resources::new()),
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
        assert!(content_type_for_image(&mime).is_none());
    }

    #[test]
    fn strips_mime_type_parameters() {
        // Issue #699
        let mime = Mime::from_str("image/png;charset=utf-8").unwrap();
        assert_eq!(
            content_type_for_image(&mime),
            Some(String::from("image/png"))
        );
    }
}
