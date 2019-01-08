use std::cell::RefCell;
use std::collections::HashMap;

use gio;

use css::CssStyles;
use defs::{Defs, Fragment};
use error::LoadingError;
use handle::{LoadOptions, RsvgHandle};
use node::RsvgNode;
use tree::Tree;
use xml::XmlState;
use xml2_load::xml_state_load_from_possibly_compressed_stream;

/// A loaded SVG file and its derived data
///
/// This contains the tree of nodes (SVG elements), the mapping
/// of id to node, and the CSS styles defined for this SVG.
pub struct Svg {
    handle: *mut RsvgHandle,

    pub tree: Tree,

    // This requires interior mutability because we load the extern
    // defs all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    pub defs: RefCell<Defs>,

    ids: HashMap<String, RsvgNode>,

    pub css_styles: CssStyles,
}

impl Svg {
    pub fn new(
        handle: *mut RsvgHandle,
        tree: Tree,
        defs: Defs,
        ids: HashMap<String, RsvgNode>,
        css_styles: CssStyles,
    ) -> Svg {
        Svg {
            handle,
            tree,
            defs: RefCell::new(defs),
            ids,
            css_styles,
        }
    }

    pub fn load_from_stream(
        load_options: LoadOptions,
        handle: *mut RsvgHandle,
        stream: gio::InputStream,
        cancellable: Option<gio::Cancellable>,
    ) -> Result<Svg, LoadingError> {
        let mut xml = XmlState::new(handle);

        xml_state_load_from_possibly_compressed_stream(
            &mut xml,
            &load_options,
            stream,
            cancellable,
        )?;

        xml.validate_tree()?;

        Ok(xml.steal_result())
    }

    pub fn lookup(&self, fragment: &Fragment) -> Option<RsvgNode> {
        if fragment.uri().is_some() {
            self.defs.borrow_mut().lookup(self.handle, fragment)
        } else {
            self.lookup_node_by_id(fragment.fragment())
        }
    }

    pub fn lookup_node_by_id(&self, id: &str) -> Option<RsvgNode> {
        self.ids.get(id).map(|n| (*n).clone())
    }
}
