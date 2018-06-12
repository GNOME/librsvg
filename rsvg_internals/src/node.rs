use cairo::{Matrix, MatrixTrait};
use downcast_rs::*;
use glib;
use glib::translate::*;
use glib_sys;
use libc;

use std::cell::{Cell, Ref, RefCell};
use std::ptr;
use std::rc::{Rc, Weak};
use std::str::FromStr;

use attributes::Attribute;
use cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use defs::{self, RsvgDefs};
use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use error::*;
use handle::RsvgHandle;
use parsers::{Parse, ParseError};
use property_bag::PropertyBag;
use state::{
    self,
    rsvg_state_new,
    ComputedValues,
    Overflow,
    RsvgState,
    SpecifiedValue,
    SpecifiedValues,
    State
};
use util::utf8_cstr;

// A *const RsvgNode is just a pointer for the C code's benefit: it
// points to an  Rc<Node>, which is our refcounted Rust representation
// of nodes.
pub type RsvgNode = Rc<Node>;

// A *const RsvgCNodeImpl is just an opaque pointer to the C code's
// struct for a particular node type.
pub enum RsvgCNodeImpl {}

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
    pub fn new(&self, node: &'a Node) -> CascadedValues<'a> {
        match self.inner {
            CascadedInner::FromNode(_) => CascadedValues {
                inner: CascadedInner::FromNode(node.values.borrow()),
            },

            CascadedInner::FromValues(ref v) => CascadedValues::new_from_values(node, v),
        }
    }

    /// Creates a `CascadedValues` that will hold the `node`'s computed values
    ///
    /// This is to be used only in the toplevel drawing function, or in elements like `<marker>`
    /// that don't propagate their parent's cascade to their children.  All others should use
    /// `new()` to derive the cascade from an existing one.
    fn new_from_node(node: &Node) -> CascadedValues {
        CascadedValues {
            inner: CascadedInner::FromNode(node.values.borrow()),
        }
    }

    /// Creates a `CascadedValues` that will override the `node`'s cascade with the specified
    /// `values`
    ///
    /// This is for the `<use>` element, which draws the element which it references with the
    /// `<use>`'s own cascade, not wih the element's original cascade.
    pub fn new_from_values(node: &'a Node, values: &ComputedValues) -> CascadedValues<'a> {
        let mut v = values.clone();
        node.get_specified_values().to_computed_values(&mut v);

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
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult;

    /// Sets any special-cased properties that the node may have, that are different
    /// from defaults in the node's `State`.
    fn set_overridden_properties(&self, _state: &mut State) {}

    fn draw(
        &self,
        _node: &RsvgNode,
        _cascaded: &CascadedValues,
        _draw_ctx: *mut RsvgDrawingCtx,
        _with_layer: bool,
        _clipping: bool,
    ) {
        // by default nodes don't draw themselves
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!(); // no Rust nodes should have this method called for them
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

pub struct Node {
    node_type: NodeType,
    parent: Option<Weak<Node>>, // optional; weak ref to parent
    id: Option<String>,         // id attribute from XML element
    class: Option<String>,      // class attribute from XML element
    first_child: RefCell<Option<Rc<Node>>>,
    last_child: RefCell<Option<Weak<Node>>>,
    next_sib: RefCell<Option<Rc<Node>>>, // next sibling; strong ref
    prev_sib: RefCell<Option<Weak<Node>>>, // previous sibling; weak ref
    state: *mut RsvgState,
    result: RefCell<NodeResult>,
    transform: Cell<Matrix>,
    values: RefCell<ComputedValues>,
    cond: Cell<bool>,
    node_impl: Box<NodeTrait>,
}

// An iterator over the Node's children
pub struct Children {
    next: Option<Rc<Node>>,
    next_back: Option<Rc<Node>>,
}

// Keep this in sync with rsvg-private.h:RsvgNodeType
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum NodeType {
    Invalid = 0,

    Chars,
    Circle,
    ClipPath,
    ComponentTransferFunction,
    Defs,
    Ellipse,
    Filter,
    Group,
    Image,
    LightSource,
    Line,
    LinearGradient,
    Link,
    Marker,
    Mask,
    Path,
    Pattern,
    Polygon,
    Polyline,
    RadialGradient,
    Rect,
    Stop,
    Svg,
    Switch,
    Symbol,
    Text,
    TRef,
    TSpan,
    Use,

    // Filter primitives
    FilterPrimitiveFirst, // just a marker; not a valid type
    FilterPrimitiveBlend,
    FilterPrimitiveColorMatrix,
    FilterPrimitiveComponentTransfer,
    FilterPrimitiveComposite,
    FilterPrimitiveConvolveMatrix,
    FilterPrimitiveDiffuseLighting,
    FilterPrimitiveDisplacementMap,
    FilterPrimitiveErode,
    FilterPrimitiveFlood,
    FilterPrimitiveGaussianBlur,
    FilterPrimitiveImage,
    FilterPrimitiveMerge,
    FilterPrimitiveMergeNode,
    FilterPrimitiveOffset,
    FilterPrimitiveSpecularLighting,
    FilterPrimitiveTile,
    FilterPrimitiveTurbulence,
    FilterPrimitiveLast, // just a marker; not a valid type
}

impl Node {
    pub fn new(
        node_type: NodeType,
        parent: Option<Weak<Node>>,
        id: Option<&str>,
        class: Option<&str>,
        state: *mut RsvgState,
        node_impl: Box<NodeTrait>,
    ) -> Node {
        Node {
            node_type,
            parent,
            id: id.map(str::to_string),
            class: class.map(str::to_string),
            first_child: RefCell::new(None),
            last_child: RefCell::new(None),
            next_sib: RefCell::new(None),
            prev_sib: RefCell::new(None),
            state,
            transform: Cell::new(Matrix::identity()),
            result: RefCell::new(Ok(())),
            values: RefCell::new(ComputedValues::default()),
            cond: Cell::new(true),
            node_impl,
        }
    }

    pub fn get_type(&self) -> NodeType {
        self.node_type
    }

    pub fn get_id(&self) -> Option<&str> {
        self.id.as_ref().map(String::as_str)
    }

    pub fn get_class(&self) -> Option<&str> {
        self.class.as_ref().map(String::as_str)
    }

    pub fn register(&self, node: &RsvgNode, defs: *mut RsvgDefs) {
        if let Some(ref id) = self.id {
            defs::register_node_by_id(defs, id, node);
        }
    }

    pub fn get_transform(&self) -> Matrix {
        self.transform.get()
    }

    pub fn get_state_mut(&self) -> &mut State {
        state::from_c_mut(self.state)
    }

    pub fn get_specified_values(&self) -> &SpecifiedValues {
        state::from_c(self.state).get_specified_values()
    }

    pub fn get_cascaded_values(&self) -> CascadedValues {
        CascadedValues::new_from_node(self)
    }

    pub fn cascade(&self, values: &ComputedValues) {
        let mut values = values.clone();
        self.get_specified_values().to_computed_values(&mut values);
        *self.values.borrow_mut() = values.clone();

        for child in self.children() {
            child.cascade(&values);
        }
    }

    pub fn get_cond(&self) -> bool {
        self.cond.get()
    }

    pub fn get_parent(&self) -> Option<Rc<Node>> {
        match self.parent {
            None => None,
            Some(ref weak_node) => Some(weak_node.upgrade().unwrap()),
        }
    }

    pub fn is_ancestor(ancestor: Rc<Node>, descendant: Rc<Node>) -> bool {
        let mut desc = Some(descendant.clone());

        while let Some(ref d) = desc.clone() {
            if rc_node_ptr_eq(&ancestor, d) {
                return true;
            }

            desc = d.get_parent();
        }

        false
    }

    pub fn add_child(&self, child: &Rc<Node>) {
        assert!(child.next_sib.borrow().is_none());
        assert!(child.prev_sib.borrow().is_none());

        if let Some(last_child_weak) = self.last_child.replace(Some(Rc::downgrade(child))) {
            if let Some(last_child) = last_child_weak.upgrade() {
                child.prev_sib.replace(Some(last_child_weak));
                last_child.next_sib.replace(Some(child.clone()));
                return;
            }
        }
        self.first_child.replace(Some(child.clone()));
    }

    pub fn set_atts(&self, node: &RsvgNode, handle: *const RsvgHandle, pbag: &PropertyBag) {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Transform => match Matrix::parse(value, ()) {
                    Ok(affine) => self.transform.set(affine),
                    Err(e) => {
                        self.set_error(NodeError::attribute_error(Attribute::Transform, e));
                        return;
                    }
                },

                _ => (),
            }
        }

        match self.parse_conditional_processing_attributes(pbag) {
            Ok(_) => (),
            Err(e) => {
                self.set_error(e);
                return;
            }
        }

        *self.result.borrow_mut() = self.node_impl.set_atts(node, handle, pbag);
    }

    fn parse_conditional_processing_attributes(&self, pbag: &PropertyBag) -> Result<(), NodeError> {
        let mut cond = self.cond.get();

        for (_key, attr, value) in pbag.iter() {
            // FIXME: move this to "do catch" when we can bump the rustc version dependency
            let mut parse = || {
                match attr {
                    Attribute::RequiredExtensions if cond => {
                        cond = RequiredExtensions::parse(value, ())
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    Attribute::RequiredFeatures if cond => {
                        cond = RequiredFeatures::parse(value, ()).map(|RequiredFeatures(res)| res)?;
                    }

                    Attribute::SystemLanguage if cond => {
                        cond = SystemLanguage::parse(value, &glib::get_language_names())
                            .map(|SystemLanguage(res, _)| res)?;
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

    pub fn set_overridden_properties(&self) {
        let mut state = self.get_state_mut();
        self.node_impl.set_overridden_properties(&mut state);
    }

    pub fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        with_layer: bool,
        clipping: bool,
    ) {
        if self.result.borrow().is_ok() {
            let cr = drawing_ctx::get_cairo_context(draw_ctx);
            let save_affine = cr.get_matrix();

            cr.transform(self.get_transform());

            self.node_impl
                .draw(node, cascaded, draw_ctx, with_layer, clipping);

            cr.set_matrix(save_affine);
        }
    }

    pub fn set_error(&self, error: NodeError) {
        *self.result.borrow_mut() = Err(error);
    }

    pub fn is_in_error(&self) -> bool {
        self.result.borrow().is_err()
    }

    pub fn get_result(&self) -> NodeResult {
        self.result.borrow().clone()
    }

    pub fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.node_impl.get_c_impl()
    }

    pub fn with_impl<T, F, U>(&self, f: F) -> U
    where
        T: NodeTrait,
        F: FnOnce(&T) -> U,
    {
        if let Some(t) = (&self.node_impl).downcast_ref::<T>() {
            f(t)
        } else {
            panic!("could not downcast");
        }
    }

    pub fn get_impl<T: NodeTrait>(&self) -> Option<&T> {
        (&self.node_impl).downcast_ref::<T>()
    }

    pub fn draw_children(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        with_layer: bool,
        clipping: bool,
    ) {
        let values = cascaded.get();

        if with_layer {
            drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);
        }

        for child in self.children() {
            drawing_ctx::draw_node_from_stack(
                draw_ctx,
                &CascadedValues::new(cascaded, &child),
                &child,
                true,
                clipping,
            );
        }

        if with_layer {
            drawing_ctx::pop_discrete_layer(draw_ctx, node, values, clipping);
        }
    }

    pub fn children(&self) -> Children {
        let last_child = self
            .last_child
            .borrow()
            .as_ref()
            .and_then(|child_weak| child_weak.upgrade());
        Children::new(self.first_child.borrow().clone(), last_child)
    }

    pub fn has_children(&self) -> bool {
        self.first_child.borrow().is_some()
    }

    pub fn set_overflow_hidden(&self) {
        let state = self.get_state_mut();
        state.values.overflow = SpecifiedValue::Specified(Overflow::Hidden);
    }
}

// Sigh, rsvg_state_free() is only available if we are being linked into
// librsvg.so.  In testing mode, we run standalone, so we omit this.
// Fortunately, in testing mode we don't create "real" nodes with
// states; we only create stub nodes with ptr::null() for state.
#[cfg(not(test))]
impl Drop for Node {
    fn drop(&mut self) {
        extern "C" {
            fn rsvg_state_free(state: *mut RsvgState);
        }
        unsafe {
            rsvg_state_free(self.state);
        }
    }
}

pub fn node_ptr_to_weak(raw_parent: *const RsvgNode) -> Option<Weak<Node>> {
    if raw_parent.is_null() {
        None
    } else {
        let p: &RsvgNode = unsafe { &*raw_parent };
        Some(Rc::downgrade(&p.clone()))
    }
}

pub fn boxed_node_new(
    node_type: NodeType,
    raw_parent: *const RsvgNode,
    id: Option<&str>,
    class: Option<&str>,
    node_impl: Box<NodeTrait>,
) -> *mut RsvgNode {
    box_node(Rc::new(Node::new(
        node_type,
        node_ptr_to_weak(raw_parent),
        id,
        class,
        rsvg_state_new(),
        node_impl,
    )))
}

impl Children {
    fn new(next: Option<Rc<Node>>, next_back: Option<Rc<Node>>) -> Self {
        Self { next, next_back }
    }

    // true if self.next_back's next sibling is self.next
    fn finished(&self) -> bool {
        match &self.next_back {
            &Some(ref next_back) => {
                next_back
                    .next_sib
                    .borrow()
                    .clone()
                    .map(|rc| &*rc as *const Node)
                    == self.next.clone().map(|rc| &*rc as *const Node)
            }
            _ => true,
        }
    }
}

impl Iterator for Children {
    type Item = Rc<Node>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished() {
            return None;
        }
        self.next.take().and_then(|next| {
            self.next = next.next_sib.borrow().clone();
            Some(next)
        })
    }
}

impl DoubleEndedIterator for Children {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.finished() {
            return None;
        }
        self.next_back.take().and_then(|next_back| {
            self.next_back = next_back
                .prev_sib
                .borrow()
                .as_ref()
                .and_then(|sib_weak| sib_weak.upgrade());
            Some(next_back)
        })
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_get_type(raw_node: *const RsvgNode) -> NodeType {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    node.get_type()
}

pub fn box_node(node: RsvgNode) -> *mut RsvgNode {
    Box::into_raw(Box::new(node))
}

#[no_mangle]
pub extern "C" fn rsvg_node_get_parent(raw_node: *const RsvgNode) -> *const RsvgNode {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    match node.get_parent() {
        None => ptr::null(),

        Some(node) => box_node(node),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_ref(raw_node: *mut RsvgNode) -> *mut RsvgNode {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    box_node(node.clone())
}

#[no_mangle]
pub extern "C" fn rsvg_node_unref(raw_node: *mut RsvgNode) -> *mut RsvgNode {
    if !raw_node.is_null() {
        let _ = unsafe { Box::from_raw(raw_node) };
    }

    // so the caller can do "node = rsvg_node_unref (node);" and lose access to the node
    ptr::null_mut()
}

// See https://github.com/rust-lang/rust/issues/36497 - this is what
// added Rc::ptr_eq(), but we don't want to depend on unstable Rust
// just yet.
fn rc_node_ptr_eq<T: ?Sized>(this: &Rc<T>, other: &Rc<T>) -> bool {
    let this_ptr: *const T = &**this;
    let other_ptr: *const T = &**other;
    this_ptr == other_ptr
}

#[no_mangle]
pub extern "C" fn rsvg_node_is_same(
    raw_node1: *const RsvgNode,
    raw_node2: *const RsvgNode,
) -> glib_sys::gboolean {
    let is_same = if raw_node1.is_null() && raw_node2.is_null() {
        true
    } else if !raw_node1.is_null() && !raw_node2.is_null() {
        let node1: &RsvgNode = unsafe { &*raw_node1 };
        let node2: &RsvgNode = unsafe { &*raw_node2 };

        rc_node_ptr_eq(node1, node2)
    } else {
        false
    };

    is_same.to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_node_add_child(raw_node: *mut RsvgNode, raw_child: *const RsvgNode) {
    assert!(!raw_node.is_null());
    assert!(!raw_child.is_null());
    let node: &mut RsvgNode = unsafe { &mut *raw_node };
    let child: &RsvgNode = unsafe { &*raw_child };

    node.add_child(child);
}

#[no_mangle]
pub extern "C" fn rsvg_node_register_in_defs(raw_node: *const RsvgNode, defs: *mut RsvgDefs) {
    assert!(!raw_node.is_null());
    assert!(!defs.is_null());

    let node: &RsvgNode = unsafe { &*raw_node };

    node.register(node, defs);
}

#[no_mangle]
pub extern "C" fn rsvg_node_set_atts(
    raw_node: *mut RsvgNode,
    handle: *const RsvgHandle,
    pbag: *const PropertyBag,
) {
    assert!(!raw_node.is_null());
    assert!(!pbag.is_null());

    let node: &RsvgNode = unsafe { &*raw_node };
    let pbag = unsafe { &*pbag };

    node.set_atts(node, handle, pbag);
}

#[no_mangle]
pub extern "C" fn rsvg_node_set_overridden_properties(raw_node: *mut RsvgNode) {
    assert!(!raw_node.is_null());

    let node: &RsvgNode = unsafe { &*raw_node };

    node.set_overridden_properties();
}

#[no_mangle]
pub extern "C" fn rsvg_node_set_attribute_parse_error(
    raw_node: *const RsvgNode,
    attr_name: *const libc::c_char,
    description: *const libc::c_char,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!attr_name.is_null());
    assert!(!description.is_null());

    unsafe {
        let attr_name = utf8_cstr(attr_name);
        let attr = Attribute::from_str(attr_name).unwrap();

        node.set_error(NodeError::parse_error(
            attr,
            ParseError::new(&String::from_glib_none(description)),
        ));
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_children_iter_begin(raw_node: *const RsvgNode) -> *mut Children {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    Box::into_raw(Box::new(node.children()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_children_iter_end(iter: *mut Children) {
    assert!(!iter.is_null());

    unsafe { Box::from_raw(iter) };
}

#[no_mangle]
pub extern "C" fn rsvg_node_children_iter_next(
    iter: *mut Children,
    out_child: *mut *mut RsvgNode,
) -> glib_sys::gboolean {
    assert!(!iter.is_null());

    let iter = unsafe { &mut *iter };
    if let Some(child) = iter.next() {
        unsafe {
            *out_child = box_node(child);
        }
        true.to_glib()
    } else {
        unsafe {
            *out_child = ptr::null_mut();
        }
        false.to_glib()
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_children_iter_next_back(
    iter: *mut Children,
    out_child: *mut *mut RsvgNode,
) -> glib_sys::gboolean {
    assert!(!iter.is_null());

    let iter = unsafe { &mut *iter };
    if let Some(child) = iter.next_back() {
        unsafe {
            *out_child = box_node(child);
        }
        true.to_glib()
    } else {
        unsafe {
            *out_child = ptr::null_mut();
        }
        false.to_glib()
    }
}

#[no_mangle]
pub extern "C" fn rsvg_root_node_cascade(raw_node: *const RsvgNode) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    let values = ComputedValues::default();

    node.cascade(&values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handle::RsvgHandle;
    use std::rc::Rc;
    use std::{mem, ptr};

    struct TestNodeImpl {}

    impl NodeTrait for TestNodeImpl {
        fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
            Ok(())
        }
    }

    #[test]
    fn node_refs_and_unrefs() {
        let node = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let ref1 = box_node(node);

        let new_node: &mut RsvgNode = unsafe { &mut *ref1 };
        let weak = Rc::downgrade(new_node);

        let ref2 = rsvg_node_ref(new_node);
        assert!(weak.upgrade().is_some());

        rsvg_node_unref(ref2);
        assert!(weak.upgrade().is_some());

        rsvg_node_unref(ref1);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn reffed_node_is_same_as_original_node() {
        let node = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let ref1 = box_node(node);

        let ref2 = rsvg_node_ref(ref1);

        assert!(rsvg_node_is_same(ref1, ref2) == true.to_glib());

        rsvg_node_unref(ref1);
        rsvg_node_unref(ref2);
    }

    #[test]
    fn different_nodes_have_different_pointers() {
        let node1 = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let ref1 = box_node(node1);

        let node2 = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let ref2 = box_node(node2);

        assert!(rsvg_node_is_same(ref1, ref2) == false.to_glib());

        rsvg_node_unref(ref1);
        rsvg_node_unref(ref2);
    }

    #[test]
    fn node_is_its_own_ancestor() {
        let node = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        assert!(Node::is_ancestor(node.clone(), node.clone()));
    }

    #[test]
    fn node_is_ancestor_of_child() {
        let node = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let child = Rc::new(Node::new(
            NodeType::Path,
            Some(Rc::downgrade(&node)),
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        node.add_child(&child);

        assert!(Node::is_ancestor(node.clone(), child.clone()));
        assert!(!Node::is_ancestor(child.clone(), node.clone()));
    }

    #[test]
    fn node_children_iterator() {
        let node = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let child = Rc::new(Node::new(
            NodeType::Path,
            Some(Rc::downgrade(&node)),
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let second_child = Rc::new(Node::new(
            NodeType::Path,
            Some(Rc::downgrade(&node)),
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        node.add_child(&child);
        node.add_child(&second_child);

        let mut children = node.children();

        let c = children.next();
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(rc_node_ptr_eq(&c, &child));

        let c = children.next_back();
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(rc_node_ptr_eq(&c, &second_child));

        assert!(children.next().is_none());
        assert!(children.next_back().is_none());
    }

    #[test]
    fn node_children_iterator_c() {
        let node = Rc::new(Node::new(
            NodeType::Path,
            None,
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let child = Rc::new(Node::new(
            NodeType::Path,
            Some(Rc::downgrade(&node)),
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        let second_child = Rc::new(Node::new(
            NodeType::Path,
            Some(Rc::downgrade(&node)),
            None,
            None,
            ptr::null_mut(),
            Box::new(TestNodeImpl {}),
        ));

        node.add_child(&child);
        node.add_child(&second_child);

        let iter = rsvg_node_children_iter_begin(&node);
        let mut c = unsafe { mem::uninitialized() };

        let result: bool = from_glib(rsvg_node_children_iter_next(iter, &mut c));
        assert_eq!(result, true);
        assert!(rc_node_ptr_eq(unsafe { &*c }, &child));
        rsvg_node_unref(c);

        let result: bool = from_glib(rsvg_node_children_iter_next_back(iter, &mut c));
        assert_eq!(result, true);
        assert!(rc_node_ptr_eq(unsafe { &*c }, &second_child));
        rsvg_node_unref(c);

        let result: bool = from_glib(rsvg_node_children_iter_next(iter, &mut c));
        assert_eq!(result, false);
        let result: bool = from_glib(rsvg_node_children_iter_next_back(iter, &mut c));
        assert_eq!(result, false);
    }
}
