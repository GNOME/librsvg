use ::glib::translate::*;
use ::libc;

use std::cell::RefCell;
use std::cell::Cell;
use std::ptr;

use cairo::MatrixTrait;

use aspect_ratio::*;
use drawing_ctx::RsvgDrawingCtx;
use drawing_ctx;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use property_bag;
use property_bag::*;
use util::*;
use viewbox::*;

/***** NodeGroup *****/

struct NodeGroup ();

impl NodeGroup {
    fn new () -> NodeGroup {
        NodeGroup ()
    }
}

impl NodeTrait for NodeGroup {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) -> NodeResult {
        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        node.draw_children (draw_ctx, dominate);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** NodeDefs *****/

struct NodeDefs ();

impl NodeDefs {
    fn new () -> NodeDefs {
        NodeDefs ()
    }
}

impl NodeTrait for NodeDefs {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) -> NodeResult {
        Ok (())
    }

    fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** NodeSwitch *****/

struct NodeSwitch ();

impl NodeSwitch {
    fn new () -> NodeSwitch {
        NodeSwitch ()
    }
}

impl NodeTrait for NodeSwitch {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, _: *const RsvgPropertyBag) -> NodeResult {
        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        drawing_ctx::state_reinherit_top (draw_ctx, node.get_state (), dominate);

        drawing_ctx::push_discrete_layer (draw_ctx);

        for child in &*node.children.borrow () {
            if drawing_ctx::state_get_cond_true (child.get_state ()) {
                let boxed_child = box_node (child.clone ());

                drawing_ctx::draw_node_from_stack (draw_ctx, boxed_child, 0);

                rsvg_node_unref (boxed_child);

                break;
            }
        }

        drawing_ctx::pop_discrete_layer (draw_ctx);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** NodeSvg *****/

struct NodeSvg {
    preserve_aspect_ratio: Cell<AspectRatio>,
    x:                     Cell<RsvgLength>,
    y:                     Cell<RsvgLength>,
    w:                     Cell<RsvgLength>,
    h:                     Cell<RsvgLength>,
    vbox:                  Cell<Option<ViewBox>>,
    atts:                  Cell<*mut RsvgPropertyBag>
}

impl NodeSvg {
    fn new () -> NodeSvg {
        NodeSvg {
            preserve_aspect_ratio: Cell::new (AspectRatio::default ()),
            x:                     Cell::new (RsvgLength::parse ("0", LengthDir::Horizontal).unwrap ()),
            y:                     Cell::new (RsvgLength::parse ("0", LengthDir::Vertical).unwrap ()),
            w:                     Cell::new (RsvgLength::parse ("100%", LengthDir::Horizontal).unwrap ()),
            h:                     Cell::new (RsvgLength::parse ("100%", LengthDir::Vertical).unwrap ()),
            vbox:                  Cell::new (None),
            atts:                  Cell::new (ptr::null_mut ())
        }
    }
}

fn length_is_negative (length: &RsvgLength) -> bool {
    // This is more or less a hack.  We don't care about a correct
    // normalization; we just need to know if it would be negative.
    // So, we pass bogus values just to be able to normalize.
    length.hand_normalize (1.0, 1.0, 1.0) < 0.0
}

impl NodeTrait for NodeSvg {
    fn set_atts (&self, node: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        self.preserve_aspect_ratio.set (property_bag::parse_or_default (pbag, "preserveAspectRatio")?);

        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        if node.get_parent ().is_some () {
            self.x.set (property_bag::length_or_default (pbag, "x", LengthDir::Horizontal)?);
            self.y.set (property_bag::length_or_default (pbag, "y", LengthDir::Vertical)?);
        }

        self.w.set (property_bag::length_or_value (pbag, "width", LengthDir::Horizontal, "100%")
                    .and_then (|l| l.check_nonnegative ()
                               .map_err (|e| NodeError::attribute_error ("width", e)))?);

        self.h.set (property_bag::length_or_value (pbag, "height", LengthDir::Vertical, "100%")
                    .and_then (|l| l.check_nonnegative ()
                               .map_err (|e| NodeError::attribute_error ("height", e)))?);

        self.vbox.set (property_bag::parse_or_none (pbag, "viewBox")?);

        // The "style" sub-element is not loaded yet here, so we need
        // to store other attributes to be applied later.
        self.atts.set (property_bag::dup (pbag));

        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        let nx = self.x.get ().normalize (draw_ctx);
        let ny = self.y.get ().normalize (draw_ctx);
        let nw = self.w.get ().normalize (draw_ctx);
        let nh = self.h.get ().normalize (draw_ctx);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#SVGElementWidthAttribute
        if double_equals (nw, 0.0) || double_equals (nh, 0.0) {
            return;
        }

        drawing_ctx::state_reinherit_top (draw_ctx, node.get_state (), dominate);

        let state = drawing_ctx::get_current_state (draw_ctx);

        let affine_old = drawing_ctx::get_current_state_affine (draw_ctx);

        if let Some (vbox) = self.vbox.get () {
            // viewBox width==0 or height==0 disables rendering of the element
            // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
            if double_equals (vbox.0.width, 0.0) || double_equals (vbox.0.height, 0.0) {
                return;
            }

            let (x, y, w, h) = self.preserve_aspect_ratio.get ().compute (vbox.0.width, vbox.0.height,
                                                                          nx, ny, nw, nh);

            let mut affine = affine_old;
            affine.translate (x, y);
            affine.scale (w / vbox.0.width, h / vbox.0.height);
            affine.translate (-vbox.0.x, -vbox.0.y);
            drawing_ctx::set_current_state_affine (draw_ctx, affine);

            drawing_ctx::push_view_box (draw_ctx, vbox.0.width, vbox.0.height);
        } else {
            let mut affine = affine_old;
            affine.translate (nx, ny);

            drawing_ctx::set_current_state_affine (draw_ctx, affine);
            drawing_ctx::push_view_box (draw_ctx, nw, nh);
        }

        let affine_new = drawing_ctx::get_current_state_affine (draw_ctx);

        drawing_ctx::push_discrete_layer (draw_ctx);

        // Bounding box addition must be AFTER the discrete layer
        // push, which must be AFTER the transformation happens.

        if !drawing_ctx::state_is_overflow (state) && node.get_parent ().is_some () {
            drawing_ctx::set_current_state_affine (draw_ctx, affine_old);
            drawing_ctx::add_clipping_rect (draw_ctx, nx, ny, nw, nh);
            drawing_ctx::set_current_state_affine (draw_ctx, affine_new);
        }

        node.draw_children (draw_ctx, -1); // dominate==-1 so it won't reinherit or push a layer

        drawing_ctx::pop_discrete_layer (draw_ctx);
        drawing_ctx::pop_view_box (draw_ctx);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

impl Drop for NodeSvg {
    fn drop (&mut self) {
        let pbag = self.atts.get ();

        if !pbag.is_null () {
            property_bag::free (pbag);
        }
    }
}

/***** NodeUse *****/

struct NodeUse {
    link: RefCell<Option<String>>,
    x:    Cell<RsvgLength>,
    y:    Cell<RsvgLength>,
    w:    Cell<Option<RsvgLength>>,
    h:    Cell<Option<RsvgLength>>,
}

impl NodeUse {
    fn new () -> NodeUse {
        NodeUse {
            link: RefCell::new (None),
            x:    Cell::new (RsvgLength::default ()),
            y:    Cell::new (RsvgLength::default ()),
            w:    Cell::new (None),
            h:    Cell::new (None)
        }
    }
}

impl NodeTrait for NodeUse {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        *self.link.borrow_mut () = property_bag::lookup (pbag, "xlink:href");

        self.x.set (property_bag::length_or_default (pbag, "x", LengthDir::Horizontal)?);
        self.y.set (property_bag::length_or_default (pbag, "y", LengthDir::Vertical)?);

        let opt_w = property_bag::length_or_none (pbag, "width", LengthDir::Horizontal)?;
        match opt_w {
            Some (w) => {
                if length_is_negative (&w) {
                    return Err (NodeError::value_error ("width", "Must not be negative"));
                } else {
                    self.w.set (Some (w));
                }
            },

            None => {
                self.w.set (None);
            }
        }

        let opt_h = property_bag::length_or_none (pbag, "height", LengthDir::Vertical)?;
        match opt_h {
            Some (h) => {
                if length_is_negative (&h) {
                    return Err (NodeError::value_error ("height", "Must not be negative"));
                } else {
                    self.h.set (Some (h));
                }
            },

            None => {
                self.h.set (None);
            }
        }

        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        let link = self.link.borrow ();

        if link.is_none () {
            return;
        }

        let raw_child = drawing_ctx::acquire_node (draw_ctx, link.as_ref ().unwrap ());
        if raw_child.is_null () {
            return;
        }

        let child: &RsvgNode = unsafe { &*raw_child };
        if Node::is_ancestor (node.clone (), child.clone ()) {
            // or, if we're <use>'ing ourselves
            drawing_ctx::release_node (draw_ctx, raw_child);
            return;
        }

        let nx = self.x.get ().normalize (draw_ctx);
        let ny = self.y.get ().normalize (draw_ctx);

        // If attributes ‘width’ and/or ‘height’ are not specified,
        // [...] use values of '100%' for these attributes.
        // From https://www.w3.org/TR/SVG/struct.html#UseElement in
        // "If the ‘use’ element references a ‘symbol’ element"
        
        let nw = self.w.get ().unwrap_or (RsvgLength::parse ("100%", LengthDir::Horizontal).unwrap ())
            .normalize (draw_ctx);
        let nh = self.h.get ().unwrap_or (RsvgLength::parse ("100%", LengthDir::Vertical).unwrap ())
            .normalize (draw_ctx);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if double_equals (nw, 0.0) {
            drawing_ctx::release_node (draw_ctx, raw_child);
            return;
        }
        
        if double_equals (nh, 0.0) {
            drawing_ctx::release_node (draw_ctx, raw_child);
            return;
        }

        drawing_ctx::state_reinherit_top (draw_ctx, node.get_state (), dominate);

        let state = drawing_ctx::get_current_state (draw_ctx);

        if child.get_type () != NodeType::Symbol {
            let mut affine = drawing_ctx::get_current_state_affine (draw_ctx);
            affine.translate (nx, ny);
            drawing_ctx::set_current_state_affine (draw_ctx, affine);

            let boxed_child = box_node (child.clone ());

            drawing_ctx::push_discrete_layer (draw_ctx);
            drawing_ctx::draw_node_from_stack (draw_ctx, boxed_child, 1);
            drawing_ctx::pop_discrete_layer (draw_ctx);
        } else {
            child.with_impl (|symbol: &NodeSymbol| {
                if let Some (vbox) = symbol.vbox.get () {
                    let (x, y, w, h) = symbol.preserve_aspect_ratio.get ().compute (vbox.0.width, vbox.0.height,
                                                                                    nx, ny, nw, nh);

                    let mut affine = drawing_ctx::get_current_state_affine (draw_ctx);
                    affine.translate (x, y);
                    affine.scale (w / vbox.0.width, h / vbox.0.height);
                    affine.translate (-vbox.0.x, -vbox.0.y);
                    drawing_ctx::set_current_state_affine (draw_ctx, affine);

                    drawing_ctx::push_view_box (draw_ctx, vbox.0.width, vbox.0.height);

                    drawing_ctx::push_discrete_layer (draw_ctx);

                    if !drawing_ctx::state_is_overflow (state) || (!drawing_ctx::state_has_overflow (state)
                                                                   && drawing_ctx::state_is_overflow (child.get_state ())) {
                        drawing_ctx::add_clipping_rect (draw_ctx, vbox.0.x, vbox.0.y, vbox.0.width, vbox.0.height);
                    }
                } else {
                    let mut affine = drawing_ctx::get_current_state_affine (draw_ctx);
                    affine.translate (nx, ny);
                    drawing_ctx::set_current_state_affine (draw_ctx, affine);

                    drawing_ctx::push_discrete_layer (draw_ctx);
                    drawing_ctx::push_view_box (draw_ctx, nw, nh);
                }

                drawing_ctx::state_push (draw_ctx);

                child.draw_children (draw_ctx, 1);

                drawing_ctx::state_pop (draw_ctx);
                drawing_ctx::pop_discrete_layer (draw_ctx);

                drawing_ctx::pop_view_box (draw_ctx);
            });
        }

        drawing_ctx::release_node (draw_ctx, raw_child);
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** NodeSymbol *****/

struct NodeSymbol {
    preserve_aspect_ratio: Cell<AspectRatio>,
    vbox:                  Cell<Option<ViewBox>>
}

impl NodeSymbol {
    fn new () -> NodeSymbol {
        NodeSymbol {
            preserve_aspect_ratio: Cell::new (AspectRatio::default ()),
            vbox:                  Cell::new (None)
        }
    }
}

impl NodeTrait for NodeSymbol {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        self.preserve_aspect_ratio.set (property_bag::parse_or_default (pbag, "preserveAspectRatio")?);
        self.vbox.set (property_bag::parse_or_none (pbag, "viewBox")?);

        Ok (())
    }

    fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

/***** C Prototypes *****/

#[no_mangle]
pub extern fn rsvg_node_group_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Group,
                    raw_parent,
                    Box::new (NodeGroup::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_defs_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Defs,
                    raw_parent,
                    Box::new (NodeDefs::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_switch_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Switch,
                    raw_parent,
                    Box::new (NodeSwitch::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_svg_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Svg,
                    raw_parent,
                    Box::new (NodeSvg::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_use_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Use,
                    raw_parent,
                    Box::new (NodeUse::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_symbol_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Symbol,
                    raw_parent,
                    Box::new (NodeSymbol::new ()))
}

#[no_mangle]
pub extern fn rsvg_node_svg_get_size (raw_node: *const RsvgNode, out_width: *mut RsvgLength, out_height: *mut RsvgLength) {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert! (!out_width.is_null ());
    assert! (!out_height.is_null ());

    node.with_impl (|svg: &NodeSvg| {
        unsafe {
            *out_width  = svg.w.get ();
            *out_height = svg.h.get ();
        }
    });
}

#[no_mangle]
pub extern fn rsvg_node_svg_get_view_box (raw_node: *const RsvgNode) -> RsvgViewBox {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut vbox: Option<ViewBox> = None;

    node.with_impl (|svg: &NodeSvg| {
        vbox = svg.vbox.get ();
    });

    RsvgViewBox::from (vbox)
}

extern "C" {
    fn rsvg_parse_style_attrs (handle: *const RsvgHandle,
                               node:   *const RsvgNode,
                               tag:    *const libc::c_char,
                               class:  *const libc::c_char,
                               id:     *const libc::c_char,
                               pbag:   *const RsvgPropertyBag);
}

#[no_mangle]
pub extern fn rsvg_node_svg_apply_atts (raw_node: *const RsvgNode, handle: *const RsvgHandle) {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    node.with_impl (|svg: &NodeSvg| {
        let pbag = svg.atts.get ();

        if !pbag.is_null () {
            let class = property_bag::lookup (pbag, "class");
            let id = property_bag::lookup (pbag, "id");

            let c_class = class.to_glib_none ();
            let c_id = id.to_glib_none ();

            unsafe { rsvg_parse_style_attrs (handle,
                                             raw_node,
                                             str::to_glib_none ("svg").0,
                                             c_class.0,
                                             c_id.0,
                                             pbag); }
        }
    });
}
