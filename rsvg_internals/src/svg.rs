use gio;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;

use allowed_url::{AllowedUrl, Fragment};
use error::LoadingError;
use handle::LoadOptions;
use io;
use node::RsvgNode;
use state::ComputedValues;
use xml::XmlState;
use xml2_load::xml_state_load_from_possibly_compressed_stream;

/// A loaded SVG file and its derived data
///
/// This contains the tree of nodes (SVG elements), the mapping
/// of id to node, and the CSS styles defined for this SVG.
pub struct Svg {
    tree: RsvgNode,

    ids: HashMap<String, RsvgNode>,

    // This requires interior mutability because we load the extern
    // resources all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    externs: RefCell<Resources>,

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

    pub fn lookup(&self, fragment: &Fragment) -> Option<RsvgNode> {
        if fragment.uri().is_some() {
            self.externs
                .borrow_mut()
                .lookup(&self.load_options, fragment)
        } else {
            self.lookup_node_by_id(fragment.fragment())
        }
    }

    pub fn lookup_node_by_id(&self, id: &str) -> Option<RsvgNode> {
        self.ids.get(id).map(|n| (*n).clone())
    }
}

struct Resources {
    resources: HashMap<AllowedUrl, Rc<Svg>>,
}

impl Resources {
    pub fn new() -> Resources {
        Resources {
            resources: Default::default(),
        }
    }

    /// Returns a node referenced by a fragment ID, from an
    /// externally-loaded SVG file.
    pub fn lookup(&mut self, load_options: &LoadOptions, fragment: &Fragment) -> Option<RsvgNode> {
        if let Some(ref href) = fragment.uri() {
            // FIXME: propagate errors from the loader
            match self.get_extern_svg(load_options, href) {
                Ok(svg) => svg.lookup_node_by_id(fragment.fragment()),

                Err(()) => None,
            }
        } else {
            unreachable!();
        }
    }

    fn get_extern_svg(&mut self, load_options: &LoadOptions, href: &str) -> Result<Rc<Svg>, ()> {
        let aurl = AllowedUrl::from_href(href, load_options.base_url.as_ref()).map_err(|_| ())?;

        match self.resources.entry(aurl) {
            Entry::Occupied(e) => Ok(e.get().clone()),
            Entry::Vacant(e) => {
                // FIXME: propagate errors
                let svg = load_svg(load_options, e.key()).map_err(|_| ())?;
                let rc_svg = e.insert(Rc::new(svg));
                Ok(rc_svg.clone())
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
