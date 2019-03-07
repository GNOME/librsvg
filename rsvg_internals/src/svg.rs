use gdk_pixbuf::{PixbufLoader, PixbufLoaderExt};
use gio;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use allowed_url::{AllowedUrl, Fragment};
use error::LoadingError;
use handle::LoadOptions;
use io::{self, BinaryData};
use node::{NodeType, RsvgNode};
use properties::ComputedValues;
use structure::{IntrinsicDimensions, NodeSvg};
use surface_utils::shared_surface::SharedImageSurface;
use xml::XmlState;
use xml2_load::xml_state_load_from_possibly_compressed_stream;

/// A loaded SVG file and its derived data
///
/// This contains the tree of nodes (SVG elements), the mapping
/// of id to node, and the CSS styles defined for this SVG.
pub struct Svg {
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

impl Svg {
    pub fn new(tree: RsvgNode, ids: HashMap<String, RsvgNode>, load_options: LoadOptions) -> Svg {
        let values = ComputedValues::default();
        tree.cascade(&values);

        Svg {
            tree,
            ids,
            externs: RefCell::new(Resources::new()),
            images: RefCell::new(Images::new()),
            load_options,
        }
    }

    pub fn load_from_stream(
        load_options: &LoadOptions,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Svg, LoadingError> {
        let load_flags = load_options.flags;
        let mut xml = XmlState::new(load_options);

        xml_state_load_from_possibly_compressed_stream(&mut xml, load_flags, stream, cancellable)?;

        xml.steal_result()
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

        assert!(root.get_type() == NodeType::Svg);

        root.with_impl(|svg: &NodeSvg| svg.get_intrinsic_dimensions())
    }
}

struct Resources {
    resources: HashMap<AllowedUrl, Result<Rc<Svg>, LoadingError>>,
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
            self.get_extern_svg(load_options, href).and_then(|svg| {
                svg.lookup_node_by_id(fragment.fragment())
                    .ok_or(LoadingError::BadUrl)
            })
        } else {
            unreachable!();
        }
    }

    fn get_extern_svg(
        &mut self,
        load_options: &LoadOptions,
        href: &str,
    ) -> Result<Rc<Svg>, LoadingError> {
        let aurl = AllowedUrl::from_href(href, load_options.base_url.as_ref())
            .map_err(|_| LoadingError::BadUrl)?;

        match self.resources.entry(aurl) {
            Entry::Occupied(e) => e.get().clone(),
            Entry::Vacant(e) => {
                let svg = load_svg(load_options, e.key()).map(|s| Rc::new(s));
                let res = e.insert(svg);
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

fn load_svg(load_options: &LoadOptions, aurl: &AllowedUrl) -> Result<Svg, LoadingError> {
    // FIXME: pass a cancellable to these
    io::acquire_stream(aurl, None).and_then(|stream| {
        Svg::load_from_stream(&load_options.copy_with_base_url(aurl), &stream, None)
    })
}

fn load_image(
    load_options: &LoadOptions,
    aurl: &AllowedUrl,
) -> Result<SharedImageSurface, LoadingError> {
    let BinaryData {
        data: bytes,
        content_type,
    } = io::acquire_data(&aurl, None)?;

    if bytes.len() == 0 {
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

    let bytes = if load_options.flags.keep_image_data {
        Some(bytes)
    } else {
        None
    };

    let surface =
        SharedImageSurface::from_pixbuf(&pixbuf, bytes, content_type.as_ref().map(String::as_str))?;

    Ok(surface)
}
