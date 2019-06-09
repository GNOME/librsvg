use cairo::{Matrix, MatrixTrait};
use downcast_rs::*;
use markup5ever::{local_name, LocalName};
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashSet;
use std::fmt;

use crate::cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use crate::css::CssRules;
use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::filters::Filter;
use crate::parsers::Parse;
use crate::properties::{ComputedValues, SpecifiedValue, SpecifiedValues};
use crate::property_bag::PropertyBag;
use crate::property_defs::Overflow;
use crate::tree_utils::{NodeRef, NodeWeakRef};
use locale_config::Locale;

/// Tree node with specific data
pub type RsvgNode = NodeRef<NodeData>;
pub type RsvgWeakNode = NodeWeakRef<NodeData>;

/// Contents of a tree node
pub struct NodeData {
    node_type: NodeType,
    element_name: LocalName,
    id: Option<String>,    // id attribute from XML element
    class: Option<String>, // class attribute from XML element
    specified_values: RefCell<SpecifiedValues>,
    important_styles: RefCell<HashSet<LocalName>>,
    result: RefCell<NodeResult>,
    transform: Cell<Matrix>,
    values: RefCell<ComputedValues>,
    cond: Cell<bool>,
    node_impl: Box<NodeTrait>,
    style_attr: RefCell<String>,
}

impl NodeData {
    pub fn new(
        node_type: NodeType,
        element_name: LocalName,
        id: Option<&str>,
        class: Option<&str>,
        node_impl: Box<NodeTrait>,
    ) -> NodeData {
        NodeData {
            node_type,
            element_name,
            id: id.map(str::to_string),
            class: class.map(str::to_string),
            specified_values: RefCell::new(Default::default()),
            important_styles: Default::default(),
            transform: Cell::new(Matrix::identity()),
            result: RefCell::new(Ok(())),
            values: RefCell::new(ComputedValues::default()),
            cond: Cell::new(true),
            node_impl,
            style_attr: RefCell::new(String::new()),
        }
    }

    pub fn get_node_trait(&self) -> &NodeTrait {
        self.node_impl.as_ref()
    }

    pub fn get_impl<T: NodeTrait>(&self) -> &T {
        if let Some(t) = (&self.node_impl).downcast_ref::<T>() {
            t
        } else {
            panic!("could not downcast");
        }
    }

    pub fn get_type(&self) -> NodeType {
        self.node_type
    }

    pub fn element_name(&self) -> &str {
        self.element_name.as_ref()
    }

    pub fn get_id(&self) -> Option<&str> {
        self.id.as_ref().map(String::as_str)
    }

    pub fn get_class(&self) -> Option<&str> {
        self.class.as_ref().map(String::as_str)
    }

    pub fn get_cond(&self) -> bool {
        self.cond.get()
    }

    pub fn get_transform(&self) -> Matrix {
        self.transform.get()
    }

    pub fn is_overflow(&self) -> bool {
        self.specified_values.borrow().is_overflow()
    }

    pub fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>, locale: &Locale) {
        if self.node_impl.overflow_hidden() {
            let mut specified_values = self.specified_values.borrow_mut();
            specified_values.overflow = SpecifiedValue::Specified(Overflow::Hidden);
        }

        self.save_style_attribute(pbag);

        if let Err(e) = self
            .set_transform_attribute(pbag)
            .and_then(|_| self.set_conditional_processing_attributes(pbag, locale))
            .and_then(|_| self.node_impl.set_atts(node, pbag))
            .and_then(|_| self.set_presentation_attributes(pbag))
        {
            self.set_error(e);
        }

        let mut specified_values = self.specified_values.borrow_mut();
        self.node_impl
            .set_overridden_properties(&mut specified_values);
    }

    fn save_style_attribute(&self, pbag: &PropertyBag<'_>) {
        let mut style_attr = self.style_attr.borrow_mut();

        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("style") => style_attr.push_str(value),

                _ => (),
            }
        }
    }

    fn set_transform_attribute(&self, pbag: &PropertyBag<'_>) -> Result<(), NodeError> {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("transform") => {
                    return Matrix::parse_str(value)
                        .attribute(attr)
                        .and_then(|affine| Ok(self.transform.set(affine)));
                }

                _ => (),
            }
        }

        Ok(())
    }

    fn set_conditional_processing_attributes(
        &self,
        pbag: &PropertyBag<'_>,
        locale: &Locale,
    ) -> Result<(), NodeError> {
        let mut cond = self.cond.get();

        for (attr, value) in pbag.iter() {
            // FIXME: move this to "try {}" when we can bump the rustc version dependency
            let mut parse = || {
                match attr {
                    local_name!("requiredExtensions") if cond => {
                        cond = RequiredExtensions::from_attribute(value)
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    local_name!("requiredFeatures") if cond => {
                        cond = RequiredFeatures::from_attribute(value)
                            .map(|RequiredFeatures(res)| res)?;
                    }

                    local_name!("systemLanguage") if cond => {
                        cond = SystemLanguage::from_attribute(value, locale)
                            .map(|SystemLanguage(res)| res)?;
                    }

                    _ => {}
                }

                Ok(cond)
            };

            parse()
                .map(|c| self.cond.set(c))
                .map_err(|e| NodeError::attribute_error(attr, e))?;
        }

        Ok(())
    }

    /// Hands the pbag to the node's state, to apply the presentation attributes
    fn set_presentation_attributes(&self, pbag: &PropertyBag<'_>) -> Result<(), NodeError> {
        match self
            .specified_values
            .borrow_mut()
            .parse_presentation_attributes(pbag)
        {
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

    /// Applies the CSS rules that match into the node's specified_values
    fn set_css_styles(&self, node: &RsvgNode, css_rules: &CssRules) {
        let mut specified_values = self.specified_values.borrow_mut();
        let mut important_styles = self.important_styles.borrow_mut();

        for selector in &css_rules.get_matches(node) {
            if let Some(decl_list) = css_rules.get_declarations(selector) {
                for declaration in decl_list.iter() {
                    specified_values
                        .set_property_from_declaration(declaration, &mut important_styles);
                }
            }
        }
    }

    /// Applies CSS styles from the saved value of the "style" attribute
    fn set_style_attribute(&self) {
        let mut style_attr = self.style_attr.borrow_mut();

        if !style_attr.is_empty() {
            let mut important_styles = self.important_styles.borrow_mut();

            if let Err(e) = self
                .specified_values
                .borrow_mut()
                .parse_style_declarations(style_attr.as_str(), &mut important_styles)
            {
                self.set_error(e);
            }

            style_attr.clear();
            style_attr.shrink_to_fit();
        }
    }

    // Sets the node's specified values from the style-related attributes in the pbag.
    // Also applies CSS rules in our limited way based on the node's tag/class/id.
    pub fn set_style(&self, node: &RsvgNode, css_rules: &CssRules) {
        self.set_css_styles(node, css_rules);
        self.set_style_attribute();
    }

    fn set_error(&self, error: NodeError) {
        *self.result.borrow_mut() = Err(error);
    }

    pub fn is_in_error(&self) -> bool {
        self.result.borrow().is_err()
    }
}

impl fmt::Display for NodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} id={}",
            self.get_type(),
            self.get_id().unwrap_or("None")
        )
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
    FromNode(Ref<'a, ComputedValues>),
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
                inner: CascadedInner::FromNode(node.borrow().values.borrow()),
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
            inner: CascadedInner::FromNode(node.borrow().values.borrow()),
        }
    }

    /// Creates a `CascadedValues` that will override the `node`'s cascade with the specified
    /// `values`
    ///
    /// This is for the `<use>` element, which draws the element which it references with the
    /// `<use>`'s own cascade, not wih the element's original cascade.
    pub fn new_from_values(node: &'a RsvgNode, values: &ComputedValues) -> CascadedValues<'a> {
        let mut v = values.clone();
        node.borrow()
            .specified_values
            .borrow()
            .to_computed_values(&mut v);

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
            CascadedInner::FromNode(ref r) => &*r,
            CascadedInner::FromValues(ref v) => v,
        }
    }
}

/// The basic trait that all nodes must implement
pub trait NodeTrait: Downcast {
    /// Sets per-node attributes from the `pbag`
    ///
    /// Each node is supposed to iterate the `pbag`, and parse any attributes it needs.
    fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult;

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
        _cascaded: &CascadedValues<'_>,
        _draw_ctx: &mut DrawingCtx,
        _clipping: bool,
    ) -> Result<(), RenderingError> {
        // by default nodes don't draw themselves
        Ok(())
    }

    /// Returns the Filter trait if this node is a filter primitive
    fn as_filter(&self) -> Option<&Filter> {
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
    ComponentTransferFunctionA,
    ComponentTransferFunctionB,
    ComponentTransferFunctionG,
    ComponentTransferFunctionR,
    Defs,
    DistantLight,
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
    PointLight,
    Polygon,
    Polyline,
    RadialGradient,
    Rect,
    SpotLight,
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
    FeFlood,
    FeGaussianBlur,
    FeImage,
    FeMerge,
    FeMergeNode,
    FeMorphology,
    FeOffset,
    FeSpecularLighting,
    FeTile,
    FeTurbulence,
}

/// Helper trait for cascading recursively
pub trait NodeCascade {
    fn cascade(&self, values: &ComputedValues);
}

impl NodeCascade for RsvgNode {
    fn cascade(&self, values: &ComputedValues) {
        let mut values = values.clone();
        self.borrow()
            .specified_values
            .borrow()
            .to_computed_values(&mut values);
        *self.borrow().values.borrow_mut() = values.clone();

        for child in self.children() {
            child.cascade(&values);
        }
    }
}

/// Helper trait for drawing recursively
pub trait NodeDraw {
    fn draw(
        &self,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<(), RenderingError>;

    fn draw_children(
        &self,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<(), RenderingError>;
}

impl NodeDraw for RsvgNode {
    fn draw(
        &self,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        if !self.borrow().is_in_error() {
            draw_ctx.with_saved_matrix(&mut |dc| {
                let cr = dc.get_cairo_context();
                cr.transform(self.borrow().get_transform());

                self.borrow()
                    .get_node_trait()
                    .draw(self, cascaded, dc, clipping)
            })
        } else {
            rsvg_log!("(not rendering element {} because it is in error)", self);

            Ok(()) // maybe we should actually return a RenderingError::NodeIsInError here?
        }
    }

    fn draw_children(
        &self,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        for child in self.children() {
            draw_ctx.draw_node_from_stack(
                &CascadedValues::new(cascaded, &child),
                &child,
                clipping,
            )?;
        }

        Ok(())
    }
}
