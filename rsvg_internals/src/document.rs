use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use gio;
use markup5ever::{LocalName, Namespace, QualName};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use crate::allowed_url::{AllowedUrl, Fragment};
use crate::create_node::create_node;
use crate::css::CssRules;
use crate::error::LoadingError;
use crate::handle::LoadOptions;
use crate::io::{self, BinaryData};
use crate::node::{NodeCascade, NodeData, NodeType, RsvgNode};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::structure::{IntrinsicDimensions, Svg};
use crate::style::Style;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::text::NodeChars;
use crate::xml::xml_load_from_possibly_compressed_stream;

/// A loaded SVG file and its derived data
///
/// This contains the tree of nodes (SVG elements), the mapping
/// of id to node, and the CSS styles defined for this SVG.
pub struct Document {
    tree: RsvgNode,

    ids: HashMap<String, RsvgNode>,

    // These require interior mutability because we load the extern
    // resources all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    externs: RefCell<Resources>,
    images: RefCell<Images>,

    // Once we do not need to load externs, we can drop this as well
    load_options: LoadOptions,
}

impl Document {
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

    pub fn root(&self) -> RsvgNode {
        self.tree.clone()
    }

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

    pub fn lookup_node_by_id(&self, id: &str) -> Option<RsvgNode> {
        self.ids.get(id).map(|n| (*n).clone())
    }

    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, LoadingError> {
        self.images.borrow_mut().lookup(&self.load_options, href)
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let root = self.root();
        let node_data = root.borrow();

        assert!(node_data.get_type() == NodeType::Svg);
        node_data.get_impl::<Svg>().get_intrinsic_dimensions()
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
    pub fn new() -> Images {
        Images {
            images: Default::default(),
        }
    }

    pub fn lookup(
        &mut self,
        load_options: &LoadOptions,
        href: &str,
    ) -> Result<SharedImageSurface, LoadingError> {
        let aurl = AllowedUrl::from_href(href, load_options.base_url.as_ref())
            .map_err(|_| LoadingError::BadUrl)?;

        match self.images.entry(aurl) {
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

struct Stylesheet {
    alternate: Option<String>,
    type_: Option<String>,
    href: Option<String>,
}

pub struct DocumentBuilder {
    load_options: LoadOptions,
    tree: Option<RsvgNode>,
    ids: HashMap<String, RsvgNode>,
    stylesheets: Vec<Stylesheet>,
    css_rules: CssRules,
}

impl DocumentBuilder {
    pub fn new(load_options: &LoadOptions) -> DocumentBuilder {
        DocumentBuilder {
            load_options: load_options.clone(),
            tree: None,
            ids: HashMap::new(),
            stylesheets: Vec::new(),
            css_rules: CssRules::default(),
        }
    }

    pub fn append_stylesheet(
        &mut self,
        alternate: Option<String>,
        type_: Option<String>,
        href: Option<String>,
    ) {
        self.stylesheets.push(Stylesheet {
            alternate,
            type_,
            href,
        });
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

    pub fn append_characters(&mut self, text: &str, parent: &mut RsvgNode) {
        if text.is_empty() {
            return;
        }

        if parent.borrow().get_type() == NodeType::Style {
            if parent.borrow().get_impl::<Style>().is_text_css() {
                self.parse_css(text);
            }
        } else {
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

    pub fn resolve_href(&self, href: &str) -> Result<(AllowedUrl), LoadingError> {
        AllowedUrl::from_href(href, self.load_options.base_url.as_ref())
            .map_err(|_| LoadingError::BadUrl)
    }

    fn parse_css(&mut self, css_data: &str) {
        self.css_rules
            .parse(self.load_options.base_url.as_ref(), css_data);
    }

    pub fn build(mut self) -> Result<Document, LoadingError> {
        for s in self.stylesheets.iter() {
            if s.type_.as_ref().map(String::as_str) != Some("text/css")
                || (s.alternate.is_some() && s.alternate.as_ref().map(String::as_str) != Some("no"))
                || s.href.is_none()
            {
                return Err(LoadingError::BadStylesheet);
            }

            // FIXME: handle CSS errors
            let _ = self.css_rules.load_css(&self.resolve_href(s.href.as_ref().unwrap())?);
        }

        let DocumentBuilder { load_options, tree, ids, css_rules, .. } = self;

        match tree {
            None => Err(LoadingError::SvgHasNoElements),
            Some(mut root) => {
                if root.borrow().get_type() == NodeType::Svg {
                    for mut node in root.descendants() {
                        node.borrow_mut().set_style(&css_rules);
                    }

                    let values = ComputedValues::default();
                    root.cascade(&values);

                    Ok(Document {
                        tree: root.clone(),
                        ids: ids,
                        externs: RefCell::new(Resources::new()),
                        images: RefCell::new(Images::new()),
                        load_options: load_options.clone(),
                    })
                } else {
                    Err(LoadingError::RootElementIsNotSvg)
                }
            }
        }
    }
}
