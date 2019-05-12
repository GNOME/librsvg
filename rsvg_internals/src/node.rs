use cairo::{Matrix, MatrixTrait};
use downcast_rs::*;
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

use crate::attributes::Attribute;
use crate::cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use crate::css::CssRules;
use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::parsers::Parse;
use crate::properties::{ComputedValues, SpecifiedValue, SpecifiedValues};
use crate::property_bag::PropertyBag;
use crate::property_defs::Overflow;
use crate::tree_utils::{self, NodeRef, NodeWeakRef};
use locale_config::Locale;

/// Tree node with specific data
pub type RsvgNode = NodeRef<NodeData>;
pub type RsvgWeakNode = NodeWeakRef<NodeData>;

/// Contents of a tree node
pub struct NodeData {
    node_type: NodeType,
    id: Option<String>,    // id attribute from XML element
    class: Option<String>, // class attribute from XML element
    specified_values: RefCell<SpecifiedValues>,
    important_styles: RefCell<HashSet<Attribute>>,
    result: RefCell<NodeResult>,
    transform: Cell<Matrix>,
    values: RefCell<ComputedValues>,
    cond: Cell<bool>,
    node_impl: Box<NodeTrait>,
    style_attr: RefCell<String>,
}

impl NodeRef<NodeData> {
    pub fn new(
        node_type: NodeType,
        parent: Option<&NodeRef<NodeData>>,
        id: Option<&str>,
        class: Option<&str>,
        node_impl: Box<NodeTrait>,
    ) -> NodeRef<NodeData> {
        let data = NodeData {
            node_type,
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
        };

        NodeRef(Rc::new(tree_utils::Node::new(data, parent)))
    }

    pub fn downgrade(&self) -> RsvgWeakNode {
        Rc::downgrade(&self.0)
    }

    pub fn upgrade(weak: &RsvgWeakNode) -> Option<NodeRef<NodeData>> {
        weak.upgrade().map(NodeRef)
    }
}

impl NodeData {
    pub fn get_impl<T: NodeTrait>(&self) -> Option<&T> {
        (&self.node_impl).downcast_ref::<T>()
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
/// This structure gets created by `Node.get_cascaded_values()`.  You can then call the `get()`
/// method on the resulting `CascadedValues` to get a `&ComputedValues` whose fields you can access.
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
    fn new_from_node(node: &RsvgNode) -> CascadedValues<'_> {
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

    // Filter primitives
    FilterPrimitiveBlend,
    FilterPrimitiveColorMatrix,
    FilterPrimitiveComponentTransfer,
    FilterPrimitiveComposite,
    FilterPrimitiveConvolveMatrix,
    FilterPrimitiveDiffuseLighting,
    FilterPrimitiveDisplacementMap,
    FilterPrimitiveFlood,
    FilterPrimitiveGaussianBlur,
    FilterPrimitiveImage,
    FilterPrimitiveMerge,
    FilterPrimitiveMergeNode,
    FilterPrimitiveMorphology,
    FilterPrimitiveOffset,
    FilterPrimitiveSpecularLighting,
    FilterPrimitiveTile,
    FilterPrimitiveTurbulence,
}

impl NodeType {
    pub fn element_name(&self) -> &'static str {
        match self {
            NodeType::Chars => "rsvg-chars", // Dummy element name for chars
            NodeType::Circle => "circle",
            NodeType::ClipPath => "clipPath",
            NodeType::ComponentTransferFunctionA => "feFuncA",
            NodeType::ComponentTransferFunctionB => "feFuncB",
            NodeType::ComponentTransferFunctionG => "feFuncG",
            NodeType::ComponentTransferFunctionR => "feFuncR",
            NodeType::Defs => "defs",
            NodeType::DistantLight => "feDistantLight",
            NodeType::Ellipse => "ellipse",
            NodeType::Filter => "filter",
            NodeType::Group => "g",
            NodeType::Image => "image",
            NodeType::Line => "line",
            NodeType::LinearGradient => "linearGradient",
            NodeType::Link => "a",
            NodeType::Marker => "marker",
            NodeType::Mask => "mask",
            NodeType::Path => "path",
            NodeType::Pattern => "pattern",
            NodeType::PointLight => "fePointight",
            NodeType::Polygon => "polygon",
            NodeType::Polyline => "polyline",
            NodeType::RadialGradient => "radialGradient",
            NodeType::Rect => "rect",
            NodeType::SpotLight => "feSpotLight",
            NodeType::Stop => "stop",
            NodeType::Style => "style",
            NodeType::Svg => "svg",
            NodeType::Switch => "switch",
            NodeType::Symbol => "symbol",
            NodeType::Text => "text",
            NodeType::TRef => "tref",
            NodeType::TSpan => "tspan",
            NodeType::Use => "use",
            NodeType::FilterPrimitiveBlend => "feBlend",
            NodeType::FilterPrimitiveColorMatrix => "feColorMatrix",
            NodeType::FilterPrimitiveComponentTransfer => "feComponentTransfer",
            NodeType::FilterPrimitiveComposite => "feComposite",
            NodeType::FilterPrimitiveConvolveMatrix => "feConvolveMatrix",
            NodeType::FilterPrimitiveDiffuseLighting => "feDiffuseLighting",
            NodeType::FilterPrimitiveDisplacementMap => "feDisplacementMap",
            NodeType::FilterPrimitiveFlood => "feFlood",
            NodeType::FilterPrimitiveGaussianBlur => "feGaussianBlur",
            NodeType::FilterPrimitiveImage => "feImage",
            NodeType::FilterPrimitiveMerge => "feMerge",
            NodeType::FilterPrimitiveMergeNode => "feMergeNode",
            NodeType::FilterPrimitiveMorphology => "feMorphology",
            NodeType::FilterPrimitiveOffset => "feOffset",
            NodeType::FilterPrimitiveSpecularLighting => "feSpecularLighting",
            NodeType::FilterPrimitiveTile => "feTile",
            NodeType::FilterPrimitiveTurbulence => "feTurbulence",
        }
    }
}

impl RsvgNode {
    pub fn get_type(&self) -> NodeType {
        self.borrow().node_type
    }

    pub fn get_id(&self) -> Option<&str> {
        self.borrow().id.as_ref().map(String::as_str)
    }

    pub fn get_class(&self) -> Option<&str> {
        self.borrow().class.as_ref().map(String::as_str)
    }

    pub fn get_transform(&self) -> Matrix {
        self.borrow().transform.get()
    }

    pub fn get_cascaded_values(&self) -> CascadedValues<'_> {
        CascadedValues::new_from_node(self)
    }

    pub fn cascade(&self, values: &ComputedValues) {
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

    pub fn set_styles_recursively(&self, node: &RsvgNode, css_rules: &CssRules) {
        self.set_style(node, css_rules);

        for child in self.children() {
            child.set_styles_recursively(&child, css_rules);
        }
    }

    pub fn get_cond(&self) -> bool {
        self.borrow().cond.get()
    }

    fn set_transform_attribute(&self, pbag: &PropertyBag<'_>) -> Result<(), NodeError> {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::Transform => {
                    return Matrix::parse_str(value)
                        .attribute(Attribute::Transform)
                        .and_then(|affine| Ok(self.borrow().transform.set(affine)));
                }

                _ => (),
            }
        }

        Ok(())
    }

    fn save_style_attribute(&self, pbag: &PropertyBag<'_>) {
        let mut style_attr = self.borrow().style_attr.borrow_mut();

        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::Style => style_attr.push_str(value),

                _ => (),
            }
        }
    }

    pub fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>, locale: &Locale) {
        self.save_style_attribute(pbag);

        if let Err(e) = self
            .set_transform_attribute(pbag)
            .and_then(|_| self.parse_conditional_processing_attributes(pbag, locale))
            .and_then(|_| self.borrow().node_impl.set_atts(node, pbag))
            .and_then(|_| self.set_presentation_attributes(pbag))
        {
            self.set_error(e);
        }
    }

    fn parse_conditional_processing_attributes(
        &self,
        pbag: &PropertyBag<'_>,
        locale: &Locale,
    ) -> Result<(), NodeError> {
        let mut cond = self.get_cond();

        for (attr, value) in pbag.iter() {
            // FIXME: move this to "try {}" when we can bump the rustc version dependency
            let mut parse = || {
                match attr {
                    Attribute::RequiredExtensions if cond => {
                        cond = RequiredExtensions::from_attribute(value)
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    Attribute::RequiredFeatures if cond => {
                        cond = RequiredFeatures::from_attribute(value)
                            .map(|RequiredFeatures(res)| res)?;
                    }

                    Attribute::SystemLanguage if cond => {
                        cond = SystemLanguage::from_attribute(value, locale)
                            .map(|SystemLanguage(res)| res)?;
                    }

                    _ => {}
                }

                Ok(cond)
            };

            parse()
                .map(|c| self.borrow().cond.set(c))
                .map_err(|e| NodeError::attribute_error(attr, e))?;
        }

        Ok(())
    }

    /// Hands the pbag to the node's state, to apply the presentation attributes
    fn set_presentation_attributes(&self, pbag: &PropertyBag<'_>) -> Result<(), NodeError> {
        match self
            .borrow()
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
        let mut specified_values = self.borrow().specified_values.borrow_mut();
        let mut important_styles = self.borrow().important_styles.borrow_mut();

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
        let mut style_attr = self.borrow().style_attr.borrow_mut();

        if !style_attr.is_empty() {
            let mut important_styles = self.borrow().important_styles.borrow_mut();

            if let Err(e) = self
                .borrow()
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
    fn set_style(&self, node: &RsvgNode, css_rules: &CssRules) {
        self.set_css_styles(node, css_rules);
        self.set_style_attribute();
    }

    pub fn set_overridden_properties(&self) {
        let mut specified_values = self.borrow().specified_values.borrow_mut();
        self.borrow()
            .node_impl
            .set_overridden_properties(&mut specified_values);
    }

    pub fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        if !self.is_in_error() {
            draw_ctx.with_saved_matrix(&mut |dc| {
                let cr = dc.get_cairo_context();
                cr.transform(self.get_transform());

                self.borrow().node_impl.draw(node, cascaded, dc, clipping)
            })
        } else {
            rsvg_log!("(not rendering element {} because it is in error)", self);

            Ok(()) // maybe we should actually return a RenderingError::NodeIsInError here?
        }
    }

    pub fn set_error(&self, error: NodeError) {
        rsvg_log!("(attribute error for {}:\n  {})", self, error);

        *self.borrow().result.borrow_mut() = Err(error);
    }

    pub fn is_in_error(&self) -> bool {
        self.borrow().result.borrow().is_err()
    }

    pub fn with_impl<T, F, U>(&self, f: F) -> U
    where
        T: NodeTrait,
        F: FnOnce(&T) -> U,
    {
        if let Some(t) = (&self.borrow().node_impl).downcast_ref::<T>() {
            f(t)
        } else {
            panic!("could not downcast");
        }
    }

    pub fn get_impl<T: NodeTrait>(&self) -> Option<&T> {
        self.borrow().get_impl()
    }

    pub fn draw_children(
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

    pub fn is_overflow(&self) -> bool {
        self.borrow().specified_values.borrow().is_overflow()
    }

    pub fn set_overflow_hidden(&self) {
        let mut specified_values = self.borrow().specified_values.borrow_mut();
        specified_values.overflow = SpecifiedValue::Specified(Overflow::Hidden);
    }
}

impl fmt::Display for RsvgNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} id={}",
            self.get_type(),
            self.get_id().unwrap_or("None")
        )
    }
}
