use std::cell::RefCell;

use css::CssStyles;
use defs::{Defs, Fragment};
use handle::RsvgHandle;
use node::RsvgNode;
use tree::Tree;

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

    pub css_styles: CssStyles,
}

impl Svg {
    pub fn new(handle: *mut RsvgHandle, tree: Tree, defs: Defs, css_styles: CssStyles) -> Svg {
        Svg {
            handle,
            tree,
            defs: RefCell::new(defs),
            css_styles,
        }
    }

    pub fn lookup_node(&self, fragment: &Fragment) -> Option<RsvgNode> {
        self.defs.borrow_mut().lookup(self.handle, fragment)
    }
}
