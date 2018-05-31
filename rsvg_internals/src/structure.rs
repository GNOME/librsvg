use glib::translate::*;
use libc;

use std::cell::Cell;
use std::cell::RefCell;

use aspect_ratio::*;
use attributes::Attribute;
use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use float_eq_cairo::ApproxEqCairo;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::{parse, Parse};
use property_bag::{OwnedPropertyBag, PropertyBag};
use state::Overflow;
use viewbox::*;
use viewport::{draw_in_viewport, ClipMode};

// ************ NodeGroup ************
struct NodeGroup();

impl NodeGroup {
    fn new() -> NodeGroup {
        NodeGroup()
    }
}

impl NodeTrait for NodeGroup {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        with_layer: bool,
        clipping: bool,
    ) {
        node.draw_children(cascaded, draw_ctx, with_layer, clipping);
    }
}

// ************ NodeDefs ************
struct NodeDefs();

impl NodeDefs {
    fn new() -> NodeDefs {
        NodeDefs()
    }
}

impl NodeTrait for NodeDefs {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
        Ok(())
    }
}

// ************ NodeSwitch ************
struct NodeSwitch();

impl NodeSwitch {
    fn new() -> NodeSwitch {
        NodeSwitch()
    }
}

impl NodeTrait for NodeSwitch {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        _with_layer: bool,
        clipping: bool,
    ) {
        let values = cascaded.get();

        drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);

        if let Some(child) = node.children().find(|c| c.get_state().cond) {
            drawing_ctx::draw_node_from_stack(
                draw_ctx,
                &CascadedValues::new(cascaded, &child),
                &child,
                true,
                clipping,
            );
        }

        drawing_ctx::pop_discrete_layer(draw_ctx, values, clipping);
    }
}

// ************ NodeSvg ************
struct NodeSvg {
    preserve_aspect_ratio: Cell<AspectRatio>,
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    w: Cell<RsvgLength>,
    h: Cell<RsvgLength>,
    vbox: Cell<Option<ViewBox>>,
    pbag: RefCell<Option<OwnedPropertyBag>>,
}

impl NodeSvg {
    fn new() -> NodeSvg {
        NodeSvg {
            preserve_aspect_ratio: Cell::new(AspectRatio::default()),
            x: Cell::new(RsvgLength::parse("0", LengthDir::Horizontal).unwrap()),
            y: Cell::new(RsvgLength::parse("0", LengthDir::Vertical).unwrap()),
            w: Cell::new(RsvgLength::parse("100%", LengthDir::Horizontal).unwrap()),
            h: Cell::new(RsvgLength::parse("100%", LengthDir::Vertical).unwrap()),
            vbox: Cell::new(None),
            pbag: RefCell::new(None),
        }
    }
}

impl NodeTrait for NodeSvg {
    fn set_atts(&self, node: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        // SVG element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        let is_inner_svg = node.get_parent().is_some();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::PreserveAspectRatio => {
                    self.preserve_aspect_ratio
                        .set(parse("preserveAspectRatio", value, (), None)?)
                }

                Attribute::X => {
                    if is_inner_svg {
                        self.x.set(parse("x", value, LengthDir::Horizontal, None)?);
                    }
                }

                Attribute::Y => {
                    if is_inner_svg {
                        self.y.set(parse("y", value, LengthDir::Vertical, None)?);
                    }
                }

                Attribute::Width => self.w.set(parse(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    Some(RsvgLength::check_nonnegative),
                )?),

                Attribute::Height => self.h.set(parse(
                    "height",
                    value,
                    LengthDir::Vertical,
                    Some(RsvgLength::check_nonnegative),
                )?),

                Attribute::ViewBox => self.vbox.set(parse("viewBox", value, (), None).map(Some)?),

                _ => (),
            }
        }

        // The "style" sub-element is not loaded yet here, so we need
        // to store other attributes to be applied later.
        *self.pbag.borrow_mut() = Some(pbag.to_owned());

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        _with_layer: bool,
        clipping: bool,
    ) {
        let values = cascaded.get();

        let nx = self.x.get().normalize(values, draw_ctx);
        let ny = self.y.get().normalize(values, draw_ctx);
        let nw = self.w.get().normalize(values, draw_ctx);
        let nh = self.h.get().normalize(values, draw_ctx);

        let do_clip = !values.is_overflow() && node.get_parent().is_some();

        draw_in_viewport(
            nx,
            ny,
            nw,
            nh,
            ClipMode::ClipToViewport,
            do_clip,
            self.vbox.get(),
            self.preserve_aspect_ratio.get(),
            values,
            drawing_ctx::get_cairo_context(draw_ctx).get_matrix(),
            draw_ctx,
            clipping,
            || {
                node.draw_children(cascaded, draw_ctx, false, clipping);
            },
        );
    }
}

// ************ NodeUse ************
struct NodeUse {
    link: RefCell<Option<String>>,
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    w: Cell<Option<RsvgLength>>,
    h: Cell<Option<RsvgLength>>,
}

impl NodeUse {
    fn new() -> NodeUse {
        NodeUse {
            link: RefCell::new(None),
            x: Cell::new(RsvgLength::default()),
            y: Cell::new(RsvgLength::default()),
            w: Cell::new(None),
            h: Cell::new(None),
        }
    }
}

impl NodeTrait for NodeUse {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),

                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal, None)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical, None)?),

                Attribute::Width => self.w.set(
                    parse(
                        "width",
                        value,
                        LengthDir::Horizontal,
                        Some(RsvgLength::check_nonnegative),
                    ).map(Some)?,
                ),
                Attribute::Height => self.h.set(
                    parse(
                        "height",
                        value,
                        LengthDir::Vertical,
                        Some(RsvgLength::check_nonnegative),
                    ).map(Some)?,
                ),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        _with_layer: bool,
        clipping: bool,
    ) {
        let values = cascaded.get();

        let link = self.link.borrow();

        if link.is_none() {
            return;
        }

        let child = if let Some(acquired) =
            drawing_ctx::get_acquired_node(draw_ctx, link.as_ref().unwrap())
        {
            acquired.get()
        } else {
            return;
        };

        if Node::is_ancestor(node.clone(), child.clone()) {
            // or, if we're <use>'ing ourselves
            return;
        }

        let nx = self.x.get().normalize(values, draw_ctx);
        let ny = self.y.get().normalize(values, draw_ctx);

        // If attributes ‘width’ and/or ‘height’ are not specified,
        // [...] use values of '100%' for these attributes.
        // From https://www.w3.org/TR/SVG/struct.html#UseElement in
        // "If the ‘use’ element references a ‘symbol’ element"

        let nw = self
            .w
            .get()
            .unwrap_or_else(|| RsvgLength::parse("100%", LengthDir::Horizontal).unwrap())
            .normalize(values, draw_ctx);
        let nh = self
            .h
            .get()
            .unwrap_or_else(|| RsvgLength::parse("100%", LengthDir::Vertical).unwrap())
            .normalize(values, draw_ctx);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if nw.approx_eq_cairo(&0.0) || nh.approx_eq_cairo(&0.0) {
            return;
        }

        if child.get_type() != NodeType::Symbol {
            let cr = drawing_ctx::get_cairo_context(draw_ctx);
            cr.translate(nx, ny);

            drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);

            drawing_ctx::draw_node_from_stack(
                draw_ctx,
                &CascadedValues::new_from_values(&child, values),
                &child,
                true,
                clipping,
            );

            drawing_ctx::pop_discrete_layer(draw_ctx, values, clipping);
        } else {
            child.with_impl(|symbol: &NodeSymbol| {
                let do_clip = !values.is_overflow()
                    || (values.overflow == Overflow::Visible && child.get_state().is_overflow());

                draw_in_viewport(
                    nx,
                    ny,
                    nw,
                    nh,
                    ClipMode::ClipToVbox,
                    do_clip,
                    symbol.vbox.get(),
                    symbol.preserve_aspect_ratio.get(),
                    values,
                    drawing_ctx::get_cairo_context(draw_ctx).get_matrix(),
                    draw_ctx,
                    clipping,
                    || {
                        drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);
                        child.draw_children(
                            &CascadedValues::new_from_values(&child, values),
                            draw_ctx,
                            false,
                            clipping,
                        );
                        drawing_ctx::pop_discrete_layer(draw_ctx, values, clipping);
                    },
                );
            });
        }
    }
}

// ************ NodeSymbol ************
struct NodeSymbol {
    preserve_aspect_ratio: Cell<AspectRatio>,
    vbox: Cell<Option<ViewBox>>,
}

impl NodeSymbol {
    fn new() -> NodeSymbol {
        NodeSymbol {
            preserve_aspect_ratio: Cell::new(AspectRatio::default()),
            vbox: Cell::new(None),
        }
    }
}

impl NodeTrait for NodeSymbol {
    fn set_atts(&self, node: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        // symbol element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::PreserveAspectRatio => {
                    self.preserve_aspect_ratio
                        .set(parse("preserveAspectRatio", value, (), None)?)
                }

                Attribute::ViewBox => self.vbox.set(parse("viewBox", value, (), None).map(Some)?),

                _ => (),
            }
        }

        Ok(())
    }
}

// ************ C Prototypes ************
#[no_mangle]
pub extern "C" fn rsvg_node_group_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Group, raw_parent, Box::new(NodeGroup::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_defs_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Defs, raw_parent, Box::new(NodeDefs::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_switch_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Switch, raw_parent, Box::new(NodeSwitch::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_svg_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Svg, raw_parent, Box::new(NodeSvg::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_use_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Use, raw_parent, Box::new(NodeUse::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_symbol_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Symbol, raw_parent, Box::new(NodeSymbol::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_svg_get_size(
    raw_node: *const RsvgNode,
    out_width: *mut RsvgLength,
    out_height: *mut RsvgLength,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!out_width.is_null());
    assert!(!out_height.is_null());

    node.with_impl(|svg: &NodeSvg| unsafe {
        *out_width = svg.w.get();
        *out_height = svg.h.get();
    });
}

#[no_mangle]
pub extern "C" fn rsvg_node_svg_get_view_box(raw_node: *const RsvgNode) -> RsvgViewBox {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    let mut vbox: Option<ViewBox> = None;

    node.with_impl(|svg: &NodeSvg| {
        vbox = svg.vbox.get();
    });

    RsvgViewBox::from(vbox)
}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_parse_style_attrs(
        handle: *const RsvgHandle,
        node: *const RsvgNode,
        tag: *const libc::c_char,
        class: *const libc::c_char,
        id: *const libc::c_char,
        pbag: *const PropertyBag,
    );
}

#[no_mangle]
pub extern "C" fn rsvg_node_svg_apply_atts(raw_node: *const RsvgNode, handle: *const RsvgHandle) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    node.with_impl(|svg: &NodeSvg| {
        if let Some(owned_pbag) = svg.pbag.borrow().as_ref() {
            let pbag = PropertyBag::from_owned(owned_pbag);

            let mut class = None;
            let mut id = None;

            for (_key, attr, value) in pbag.iter() {
                match attr {
                    Attribute::Class => class = Some(value),

                    Attribute::Id => id = Some(value),

                    _ => (),
                }
            }

            let c_class = class.to_glib_none();
            let c_id = id.to_glib_none();

            unsafe {
                rsvg_parse_style_attrs(
                    handle,
                    raw_node,
                    str::to_glib_none("svg").0,
                    c_class.0,
                    c_id.0,
                    pbag.ffi(),
                );
            }
        }
    });
}
