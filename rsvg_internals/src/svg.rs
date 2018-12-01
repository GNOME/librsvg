use std::cell::RefCell;

use css::CssStyles;
use defs::Defs;
use tree::Tree;

/// A loaded SVG file and its derived data
///
/// This contains the tree of nodes (SVG elements), the mapping
/// of id to node, and the CSS styles defined for this SVG.
pub struct Svg {
    pub tree: Tree,

    // This requires interior mutability because we load the extern
    // defs all over the place.  Eventually we'll be able to do this
    // once, at loading time, and keep this immutable.
    pub defs: RefCell<Defs>,

    pub css_styles: CssStyles,
}

impl Svg {
    pub fn new(tree: Tree, defs: Defs, css_styles: CssStyles) -> Svg {
        Svg {
            tree,
            defs: RefCell::new(defs),
            css_styles,
        }
    }
}
