//! Tree nodes, the representation of SVG elements.
//!
//! Librsvg uses the [rctree crate][rctree] to represent the SVG tree of elements.
//! Its [`Node`] struct provides a generic wrapper over nodes in a tree.
//! Librsvg puts a [`NodeData`] as the type parameter of [`Node`].  For convenience,
//! librsvg has a type alias `RsvgNode = Node<NodeData>`.
//!
//! Nodes are not constructed directly by callers; this is done in the [`create_node`] module.
//!
//! [rctree]: ../../rctree/index.html
//! [`Node`]: ../../rctree/struct.Node.html
//! [`NodeData`]: struct.NodeData.html
//! [`create_node`]: ../create_node/index.html

use downcast_rs::*;
use locale_config::Locale;
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use std::cell::{Ref, RefMut};
use std::collections::HashSet;
use std::fmt;

use crate::bbox::BoundingBox;
use crate::cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use crate::css::Declaration;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::filters::FilterEffect;
use crate::parsers::Parse;
use crate::properties::{ComputedValues, SpecifiedValue, SpecifiedValues};
use crate::property_bag::PropertyBag;
use crate::property_defs::Overflow;
use crate::text::NodeChars;
use crate::transform::Transform;

/// Strong reference to an element in the SVG tree.
///
/// See the [module documentation](index.html) for more information.
pub type RsvgNode = rctree::Node<NodeData>;

/// Weak reference to an element in the SVG tree.
///
/// See the [module documentation](index.html) for more information.
pub type RsvgWeakNode = rctree::WeakNode<NodeData>;

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
/// This enum uses `Box<Element>` instead of embedding the `Element`
/// struct directly in its variant, in order to make text nodes as
/// small as possible, i.e. without all the baggage in an `Element`.
/// With the Box, the `Element` variant is the size of a pointer,
/// which is smaller than the `Text` variant.
///
/// ## Accessing the node's contents
///
/// Code that traverses the DOM tree needs to find out at runtime what
/// each node stands for.  First, use the `get_type` or `is_element`
/// methods from the `NodeBorrow` trait to see if you can then call
/// `borrow_chars`, `borrow_element`, or `borrow_element_mut`.
pub enum NodeData {
    Element(Box<Element>),
    Text(NodeChars),
}

/// Contents of an element node in the DOM
pub struct Element {
    node_type: NodeType,
    element_name: QualName,
    id: Option<String>,    // id attribute from XML element
    class: Option<String>, // class attribute from XML element
    specified_values: SpecifiedValues,
    important_styles: HashSet<QualName>,
    result: NodeResult,
    transform: Transform,
    values: ComputedValues,
    cond: bool,
    style_attr: String,
    node_impl: Box<dyn NodeTrait>,
}

impl NodeData {
    pub fn new_element(
        node_type: NodeType,
        element_name: &QualName,
        id: Option<&str>,
        class: Option<&str>,
        node_impl: Box<dyn NodeTrait>,
    ) -> NodeData {
        NodeData::Element(Box::new(Element {
            node_type,
            element_name: element_name.clone(),
            id: id.map(str::to_string),
            class: class.map(str::to_string),
            specified_values: Default::default(),
            important_styles: Default::default(),
            transform: Default::default(),
            result: Ok(()),
            values: ComputedValues::default(),
            cond: true,
            style_attr: String::new(),
            node_impl,
        }))
    }

    pub fn new_chars() -> NodeData {
        NodeData::Text(NodeChars::new())
    }

    pub fn get_type(&self) -> NodeType {
        match *self {
            NodeData::Element(ref e) => e.node_type,
            NodeData::Text(_) => NodeType::Chars,
        }
    }
}

impl Element {
    pub fn get_type(&self) -> NodeType {
        self.node_type
    }

    pub fn get_node_trait(&self) -> &dyn NodeTrait {
        self.node_impl.as_ref()
    }

    pub fn get_impl<T: NodeTrait>(&self) -> &T {
        if let Some(t) = (&self.node_impl).downcast_ref::<T>() {
            t
        } else {
            panic!("could not downcast");
        }
    }

    pub fn element_name(&self) -> &QualName {
        &self.element_name
    }

    pub fn get_id(&self) -> Option<&str> {
        self.id.as_ref().map(String::as_str)
    }

    pub fn get_class(&self) -> Option<&str> {
        self.class.as_ref().map(String::as_str)
    }

    pub fn get_cond(&self) -> bool {
        self.cond
    }

    pub fn get_transform(&self) -> Transform {
        self.transform
    }

    pub fn is_overflow(&self) -> bool {
        self.specified_values.is_overflow()
    }

    pub fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>, locale: &Locale) {
        if self.node_impl.overflow_hidden() {
            self.specified_values.overflow = SpecifiedValue::Specified(Overflow::Hidden);
        }

        self.save_style_attribute(pbag);

        if let Err(e) = self
            .set_transform_attribute(pbag)
            .and_then(|_| self.set_conditional_processing_attributes(pbag, locale))
            .and_then(|_| self.node_impl.set_atts(parent, pbag))
            .and_then(|_| self.set_presentation_attributes(pbag))
        {
            self.set_error(e);
        }

        self.node_impl
            .set_overridden_properties(&mut self.specified_values);
    }

    fn save_style_attribute(&mut self, pbag: &PropertyBag<'_>) {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "style") => self.style_attr.push_str(value),
                _ => (),
            }
        }
    }

    fn set_transform_attribute(&mut self, pbag: &PropertyBag<'_>) -> Result<(), NodeError> {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "transform") => {
                    return Transform::parse_str(value)
                        .attribute(attr)
                        .and_then(|affine| {
                            self.transform = affine;
                            Ok(())
                        });
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn set_conditional_processing_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
        locale: &Locale,
    ) -> Result<(), NodeError> {
        let mut cond = self.cond;

        for (attr, value) in pbag.iter() {
            let mut parse = || -> Result<_, ValueErrorKind> {
                match attr.expanded() {
                    expanded_name!("", "requiredExtensions") if cond => {
                        cond = RequiredExtensions::from_attribute(value)
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    expanded_name!("", "requiredFeatures") if cond => {
                        cond = RequiredFeatures::from_attribute(value)
                            .map(|RequiredFeatures(res)| res)?;
                    }

                    expanded_name!("", "systemLanguage") if cond => {
                        cond = SystemLanguage::from_attribute(value, locale)
                            .map(|SystemLanguage(res)| res)?;
                    }

                    _ => {}
                }

                Ok(cond)
            };

            parse().map(|c| self.cond = c).attribute(attr)?;
        }

        Ok(())
    }

    /// Hands the pbag to the node's state, to apply the presentation attributes
    fn set_presentation_attributes(&mut self, pbag: &PropertyBag<'_>) -> Result<(), NodeError> {
        match self.specified_values.parse_presentation_attributes(pbag) {
            Ok(_) => Ok(()),
            Err(e) => {
                // FIXME: we'll ignore errors here for now.
                //
                // If we set the node to be in error, we expose buggy handling of the
                // enable-background property; we are not parsing it correctly. This
                // causes tests/fixtures/reftests/bugs/587721-text-transform.svg to fail
                // because it has enable-background="new 0 0 1179.75118 687.74173" in the
                // toplevel svg element.
                //
                //   self.set_error(e);
                //   return;

                rsvg_log!("(attribute error: {})", e);
                Ok(())
            }
        }
    }

    // Applies a style declaration to the node's specified_values
    pub fn apply_style_declaration(&mut self, declaration: &Declaration) {
        self.specified_values
            .set_property_from_declaration(declaration, &mut self.important_styles);
    }

    /// Applies CSS styles from the saved value of the "style" attribute
    pub fn set_style_attribute(&mut self) {
        if !self.style_attr.is_empty() {
            if let Err(e) = self
                .specified_values
                .parse_style_declarations(self.style_attr.as_str(), &mut self.important_styles)
            {
                self.set_error(e);
            }

            self.style_attr.clear();
            self.style_attr.shrink_to_fit();
        }
    }

    fn set_error(&mut self, error: NodeError) {
        rsvg_log!("setting node {} in error: {}", self, error);
        self.result = Err(error);
    }

    pub fn is_in_error(&self) -> bool {
        self.result.is_err()
    }
}

impl fmt::Display for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.get_type())?;

        match *self {
            NodeData::Element(ref e) => {
                write!(f, " id={}", e.get_id().unwrap_or("None"))?;
            }

            _ => (),
        }

        Ok(())
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.get_type())?;
        write!(f, " id={}", self.get_id().unwrap_or("None"))?;
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
}

enum CascadedInner<'a> {
    FromNode(Ref<'a, Element>),
    FromValues(ComputedValues),
}

impl<'a> CascadedValues<'a> {
    /// Creates a `CascadedValues` that has the same cascading mode as &self
    ///
    /// This is what nodes should normally use to draw their children from their `draw()` method.
    /// Nodes that need to override the cascade for their children can use `new_from_values()`
    /// instead.
    pub fn new(&self, node: &'a RsvgNode) -> CascadedValues<'a> {
        match self.inner {
            CascadedInner::FromNode(_) => CascadedValues {
                inner: CascadedInner::FromNode(node.borrow_element()),
            },

            CascadedInner::FromValues(ref v) => CascadedValues::new_from_values(node, v),
        }
    }

    /// Creates a `CascadedValues` that will hold the `node`'s computed values
    ///
    /// This is to be used only in the toplevel drawing function, or in elements like `<marker>`
    /// that don't propagate their parent's cascade to their children.  All others should use
    /// `new()` to derive the cascade from an existing one.
    pub fn new_from_node(node: &RsvgNode) -> CascadedValues<'_> {
        CascadedValues {
            inner: CascadedInner::FromNode(node.borrow_element()),
        }
    }

    /// Creates a `CascadedValues` that will override the `node`'s cascade with the specified
    /// `values`
    ///
    /// This is for the `<use>` element, which draws the element which it references with the
    /// `<use>`'s own cascade, not wih the element's original cascade.
    pub fn new_from_values(node: &'a RsvgNode, values: &ComputedValues) -> CascadedValues<'a> {
        let mut v = values.clone();
        node.borrow_element().specified_values.to_computed_values(&mut v);

        CascadedValues {
            inner: CascadedInner::FromValues(v),
        }
    }

    /// Returns the cascaded `ComputedValues`.
    ///
    /// Nodes should use this from their `NodeTrait::draw()` implementation to get the
    /// `ComputedValues` from the `CascadedValues` that got passed to `draw()`.
    pub fn get(&'a self) -> &'a ComputedValues {
        match self.inner {
            CascadedInner::FromNode(ref element) => &element.values,
            CascadedInner::FromValues(ref v) => v,
        }
    }
}

/// The basic trait that all nodes must implement
pub trait NodeTrait: Downcast {
    /// Sets per-node attributes from the `pbag`
    ///
    /// Each node is supposed to iterate the `pbag`, and parse any attributes it needs.
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult;

    /// Sets any special-cased properties that the node may have, that are different
    /// from defaults in the node's `SpecifiedValues`.
    fn set_overridden_properties(&self, _values: &mut SpecifiedValues) {}

    /// Whether this node has overflow:hidden.
    /// https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
    fn overflow_hidden(&self) -> bool {
        false
    }

    fn draw(
        &self,
        _node: &RsvgNode,
        _acquired_nodes: &mut AcquiredNodes,
        _cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        _clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        // by default nodes don't draw themselves
        Ok(draw_ctx.empty_bbox())
    }

    /// Returns the FilterEffect trait if this node is a filter primitive
    fn as_filter_effect(&self) -> Option<&dyn FilterEffect> {
        None
    }
}

impl_downcast!(NodeTrait);

// After creating/parsing a Node, it will be in a success or an error state.
// We represent this with a Result, aliased as a NodeResult.  There is no
// extra information for the Ok case; all the interesting stuff is in the
// Err case.
//
// https://www.w3.org/TR/SVG/implnote.html#ErrorProcessing
//
// When an element has an error during parsing, the SVG spec calls the element
// to be "in error".  We skip rendering of elements that are in error.
//
// When we parse an element's attributes, we stop as soon as we
// encounter the first error:  a parse error, or an invalid value,
// etc.  No further attributes will be processed, although note that
// the order in which an element's attributes are processed is not
// defined.
//
// Alternatively, we could try to parse/validate all the attributes
// that come in an element and build up a Vec<NodeError>.  However, we
// don't do this now.  Doing that may be more useful for an SVG
// validator, not a renderer like librsvg is.
pub type NodeResult = Result<(), NodeError>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum NodeType {
    Chars,
    Circle,
    ClipPath,
    Ellipse,
    Filter,
    Group,
    Image,
    Line,
    LinearGradient,
    Link,
    Marker,
    Mask,
    NonRendering,
    Path,
    Pattern,
    Polygon,
    Polyline,
    RadialGradient,
    Rect,
    Stop,
    Style,
    Svg,
    Switch,
    Symbol,
    Text,
    TRef,
    TSpan,
    Use,

    // Filter primitives, these start with "Fe" as element names are e.g. "feBlend"
    FeBlend,
    FeColorMatrix,
    FeComponentTransfer,
    FeComposite,
    FeConvolveMatrix,
    FeDiffuseLighting,
    FeDisplacementMap,
    FeDistantLight,
    FeFlood,
    FeFuncA,
    FeFuncB,
    FeFuncG,
    FeFuncR,
    FeGaussianBlur,
    FeImage,
    FeMerge,
    FeMergeNode,
    FeMorphology,
    FeOffset,
    FePointLight,
    FeSpecularLighting,
    FeSpotLight,
    FeTile,
    FeTurbulence,
}

/// Helper trait to get different NodeData variants
pub trait NodeBorrow {
    /// Returns `false` for NodeData::Text, `true` otherwise.
    fn is_element(&self) -> bool;

    /// Borrows a `NodeChars` reference.
    ///
    /// Panics: will panic if `&self` is not a `NodeData::Text` node
    fn borrow_chars(&self) -> Ref<NodeChars>;

    /// Borrows an `Element` reference
    ///
    /// Panics: will panic if `&self` is not a `NodeData::Element` node
    fn borrow_element(&self) -> Ref<Element>;

    /// Borrows an `Element` reference mutably
    ///
    /// Panics: will panic if `&self` is not a `NodeData::Element` node
    fn borrow_element_mut(&mut self) -> RefMut<Element>;
}

impl NodeBorrow for RsvgNode {
    fn is_element(&self) -> bool {
        match *self.borrow() {
            NodeData::Element(_) => true,
            _ => false,
        }
    }

    fn borrow_chars(&self) -> Ref<NodeChars> {
        Ref::map(self.borrow(), |n| match *n {
            NodeData::Text(ref c) => c,
            _ => panic!("tried to borrow_chars for a non-text node"),
        })
    }

    fn borrow_element(&self) -> Ref<Element> {
        Ref::map(self.borrow(), |n| match *n {
            NodeData::Element(ref e) => e.as_ref(),
            _ => panic!("tried to borrow_element for a non-element node"),
        })
    }

    fn borrow_element_mut(&mut self) -> RefMut<Element> {
        RefMut::map(self.borrow_mut(), |n| match *n {
            NodeData::Element(ref mut e) => e.as_mut(),
            _ => panic!("tried to borrow_element_mut for a non-element node"),
        })
    }
}

/// Helper trait for cascading recursively
pub trait NodeCascade {
    fn cascade(&mut self, values: &ComputedValues);
}

impl NodeCascade for RsvgNode {
    fn cascade(&mut self, values: &ComputedValues) {
        let mut values = values.clone();

        {
            let mut elt = self.borrow_element_mut();

            elt.specified_values.to_computed_values(&mut values);
            elt.values = values.clone();
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
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError>;

    fn draw_children(
        &self,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError>;
}

impl NodeDraw for RsvgNode {
    fn draw(
        &self,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        match *self.borrow() {
            NodeData::Element(ref e) => {
                if !e.is_in_error() {
                    let transform = e.get_transform();
                    draw_ctx.with_saved_transform(Some(transform), &mut |dc| {
                        e.get_node_trait()
                            .draw(self, acquired_nodes, cascaded, dc, clipping)
                    })
                } else {
                    rsvg_log!("(not rendering element {} because it is in error)", self);

                    // maybe we should actually return a RenderingError::NodeIsInError here?
                    Ok(draw_ctx.empty_bbox())
                }
            }

            _ => Ok(draw_ctx.empty_bbox()),
        }
    }

    fn draw_children(
        &self,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let mut bbox = draw_ctx.empty_bbox();

        for child in self.children().filter(|c| c.is_element()) {
            let child_bbox = draw_ctx.draw_node_from_stack(
                &child,
                acquired_nodes,
                &CascadedValues::new(cascaded, &child),
                clipping,
            )?;
            bbox.insert(&child_bbox);
        }

        Ok(bbox)
    }
}
