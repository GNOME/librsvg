//! Tree nodes, the representation of SVG elements.
//!
//! Librsvg uses the [rctree crate][rctree] to represent the SVG tree of elements.
//! Its [`rctree::Node`] struct provides a generic wrapper over nodes in a tree.
//! Librsvg puts a [`NodeData`] as the type parameter of [`rctree::Node`].  For convenience,
//! librsvg has a type alias [`Node`]` = rctree::Node<NodeData>`.
//!
//! Nodes are not constructed directly by callers;

use markup5ever::QualName;
use std::cell::{Ref, RefMut};
use std::fmt;

use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::*;
use crate::error::*;
use crate::paint_server::PaintSource;
use crate::properties::ComputedValues;
use crate::text::Chars;
use crate::xml::Attributes;

/// Strong reference to an element in the SVG tree.
///
/// See the [module documentation][self] for more information.
pub type Node = rctree::Node<NodeData>;

/// Weak reference to an element in the SVG tree.
///
/// See the [module documentation][self] for more information.
pub type WeakNode = rctree::WeakNode<NodeData>;

/// Data for a single DOM node.
///
/// ## Memory consumption
///
/// SVG files look like this, roughly:
///
/// ```xml
/// <svg>
///   <rect x="10" y="20"/>
///   <path d="..."/>
///   <text x="10" y="20">Hello</text>
///   <!-- etc -->
/// </svg>
/// ```
///
/// Each element has a bunch of data, including the styles, which is
/// the biggest consumer of memory within the `Element` struct.  But
/// between each element there is a text node; in the example above
/// there are a bunch of text nodes with just whitespace (newlines and
/// spaces), and a single text node with "`Hello`" in it from the
/// `<text>` element.
///
/// ## Accessing the node's contents
///
/// Code that traverses the DOM tree needs to find out at runtime what
/// each node stands for.  First, use the `is_chars` or `is_element`
/// methods from the `NodeBorrow` trait to see if you can then call
/// `borrow_chars`, `borrow_element`, or `borrow_element_mut`.
pub enum NodeData {
    Element(Element),
    Text(Chars),
}

impl NodeData {
    pub fn new_element(name: &QualName, attrs: Attributes) -> NodeData {
        NodeData::Element(Element::new(name, attrs))
    }

    pub fn new_chars(initial_text: &str) -> NodeData {
        NodeData::Text(Chars::new(initial_text))
    }
}

impl fmt::Display for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            NodeData::Element(ref e) => {
                write!(f, "{}", e)?;
            }
            NodeData::Text(_) => {
                write!(f, "Chars")?;
            }
        }

        Ok(())
    }
}

/// Can obtain computed values from a node
///
/// In our tree of SVG elements (Node in our parlance), each node stores a `ComputedValues` that
/// gets computed during the initial CSS cascade.  However, sometimes nodes need to be rendered
/// outside the normal hierarchy.  For example, the `<use>` element can "instance" a subtree from
/// elsewhere in the SVG; it causes the instanced subtree to re-cascade from the computed values for
/// the `<use>` element.
///
/// You can then call the `get()` method on the resulting `CascadedValues` to get a
/// `&ComputedValues` whose fields you can access.
pub struct CascadedValues<'a> {
    inner: CascadedInner<'a>,
    pub context_stroke: Option<PaintSource>,
    pub context_fill: Option<PaintSource>,
}

enum CascadedInner<'a> {
    FromNode(Ref<'a, Element>),
    FromValues(Box<ComputedValues>),
}

impl<'a> CascadedValues<'a> {
    /// Creates a `CascadedValues` that has the same cascading mode as &self
    ///
    /// This is what nodes should normally use to draw their children from their `draw()` method.
    /// Nodes that need to override the cascade for their children can use `new_from_values()`
    /// instead.
    pub fn clone_with_node(&self, node: &'a Node) -> CascadedValues<'a> {
        match self.inner {
            CascadedInner::FromNode(_) => CascadedValues {
                inner: CascadedInner::FromNode(node.borrow_element()),
                context_fill: self.context_fill.clone(),
                context_stroke: self.context_stroke.clone(),
            },

            CascadedInner::FromValues(ref v) => CascadedValues::new_from_values(
                node,
                &*v,
                self.context_fill.clone(),
                self.context_stroke.clone(),
            ),
        }
    }

    /// Creates a `CascadedValues` that will hold the `node`'s computed values
    ///
    /// This is to be used only in the toplevel drawing function, or in elements like `<marker>`
    /// that don't propagate their parent's cascade to their children.  All others should use
    /// `new()` to derive the cascade from an existing one.
    pub fn new_from_node(node: &Node) -> CascadedValues<'_> {
        CascadedValues {
            inner: CascadedInner::FromNode(node.borrow_element()),
            context_fill: None,
            context_stroke: None,
        }
    }

    /// Creates a `CascadedValues` that will override the `node`'s cascade with the specified
    /// `values`
    ///
    /// This is for the `<use>` element, which draws the element which it references with the
    /// `<use>`'s own cascade, not with the element's original cascade.
    pub fn new_from_values(
        node: &'a Node,
        values: &ComputedValues,
        fill: Option<PaintSource>,
        stroke: Option<PaintSource>,
    ) -> CascadedValues<'a> {
        let mut v = Box::new(values.clone());
        node.borrow_element()
            .get_specified_values()
            .to_computed_values(&mut v);

        CascadedValues {
            inner: CascadedInner::FromValues(v),
            context_fill: fill,
            context_stroke: stroke,
        }
    }

    /// Returns the cascaded `ComputedValues`.
    ///
    /// Nodes should use this from their `Draw::draw()` implementation to get the
    /// `ComputedValues` from the `CascadedValues` that got passed to `draw()`.
    pub fn get(&'a self) -> &'a ComputedValues {
        match self.inner {
            CascadedInner::FromNode(ref e) => e.get_computed_values(),
            CascadedInner::FromValues(ref v) => &*v,
        }

        // if values.fill == "context-fill" {
        //     values.fill=self.context_fill
        // }
        // if values.stroke == "context-stroke" {
        //     values.stroke=self.context_stroke
        // }
    }
}

/// Helper trait to get different NodeData variants
pub trait NodeBorrow {
    /// Returns `false` for NodeData::Text, `true` otherwise.
    fn is_element(&self) -> bool;

    /// Returns `true` for NodeData::Text, `false` otherwise.
    fn is_chars(&self) -> bool;

    /// Borrows a `Chars` reference.
    ///
    /// Panics: will panic if `&self` is not a `NodeData::Text` node
    fn borrow_chars(&self) -> Ref<'_, Chars>;

    /// Borrows an `Element` reference
    ///
    /// Panics: will panic if `&self` is not a `NodeData::Element` node
    fn borrow_element(&self) -> Ref<'_, Element>;

    /// Borrows an `Element` reference mutably
    ///
    /// Panics: will panic if `&self` is not a `NodeData::Element` node
    fn borrow_element_mut(&mut self) -> RefMut<'_, Element>;
}

impl NodeBorrow for Node {
    fn is_element(&self) -> bool {
        matches!(*self.borrow(), NodeData::Element(_))
    }

    fn is_chars(&self) -> bool {
        matches!(*self.borrow(), NodeData::Text(_))
    }

    fn borrow_chars(&self) -> Ref<'_, Chars> {
        Ref::map(self.borrow(), |n| match *n {
            NodeData::Text(ref c) => c,
            _ => panic!("tried to borrow_chars for a non-text node"),
        })
    }

    fn borrow_element(&self) -> Ref<'_, Element> {
        Ref::map(self.borrow(), |n| match *n {
            NodeData::Element(ref e) => e,
            _ => panic!("tried to borrow_element for a non-element node"),
        })
    }

    fn borrow_element_mut(&mut self) -> RefMut<'_, Element> {
        RefMut::map(self.borrow_mut(), |n| match *n {
            NodeData::Element(ref mut e) => e,
            _ => panic!("tried to borrow_element_mut for a non-element node"),
        })
    }
}

#[macro_export]
macro_rules! is_element_of_type {
    ($node:expr, $element_type:ident) => {
        matches!(
            *$node.borrow_element(),
            crate::element::Element::$element_type(_)
        )
    };
}

#[macro_export]
macro_rules! borrow_element_as {
    ($node:expr, $element_type:ident) => {
        std::cell::Ref::map($node.borrow_element(), |e| match *e {
            crate::element::Element::$element_type(ref e) => &*e,
            _ => panic!("tried to borrow_element_as {}", stringify!($element_type)),
        })
    };
}

/// Helper trait for cascading recursively
pub trait NodeCascade {
    fn cascade(&mut self, values: &ComputedValues);
}

impl NodeCascade for Node {
    fn cascade(&mut self, values: &ComputedValues) {
        let mut values = values.clone();

        {
            let mut elt = self.borrow_element_mut();

            elt.get_specified_values().to_computed_values(&mut values);
            elt.set_computed_values(&values);
        }

        for mut child in self.children().filter(|c| c.is_element()) {
            child.cascade(&values);
        }
    }
}

/// Helper trait for drawing recursively
pub trait NodeDraw {
    fn draw(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError>;

    fn draw_children(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError>;
}

impl NodeDraw for Node {
    fn draw(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        match *self.borrow() {
            NodeData::Element(ref e) => e.draw(self, acquired_nodes, cascaded, draw_ctx, clipping),
            _ => Ok(draw_ctx.empty_bbox()),
        }
    }

    fn draw_children(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let mut bbox = draw_ctx.empty_bbox();

        for child in self.children().filter(|c| c.is_element()) {
            let child_bbox = draw_ctx.draw_node_from_stack(
                &child,
                acquired_nodes,
                &CascadedValues::clone_with_node(cascaded, &child),
                clipping,
            )?;
            bbox.insert(&child_bbox);
        }

        Ok(bbox)
    }
}
