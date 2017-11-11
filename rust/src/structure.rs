use ::glib::translate::*;
use ::libc;

use std::cell::RefCell;
use std::cell::Cell;
use std::ptr;

use cairo::MatrixTrait;

use aspect_ratio::*;
use drawing_ctx::RsvgDrawingCtx;
use drawing_ctx;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::Parse;
use property_bag;
use property_bag::*;
use util::*;
use viewbox::*;
use viewport::{ClipMode,draw_in_viewport};

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

impl NodeTrait for NodeSvg {
    fn set_atts (&self, node: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        self.preserve_aspect_ratio.set (property_bag::parse_or_default (pbag, "preserveAspectRatio", (), None)?);

        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        if node.get_parent ().is_some () {
            self.x.set (property_bag::parse_or_default (pbag, "x", LengthDir::Horizontal, None)?);
            self.y.set (property_bag::parse_or_default (pbag, "y", LengthDir::Vertical, None)?);
        }

        self.w.set (property_bag::parse_or_value (pbag,
                                                  "width",
                                                  LengthDir::Horizontal,
                                                  RsvgLength::parse ("100%", LengthDir::Horizontal).unwrap (),
                                                  Some(RsvgLength::check_nonnegative))?);

        self.h.set (property_bag::parse_or_value (pbag,
                                                  "height",
                                                  LengthDir::Vertical,
                                                  RsvgLength::parse ("100%", LengthDir::Vertical).unwrap (),
                                                  Some(RsvgLength::check_nonnegative))?);

        self.vbox.set (property_bag::parse_or_none (pbag, "viewBox", (), None)?);

        // The "style" sub-element is not loaded yet here, so we need
        // to store other attributes to be applied later.
        self.atts.set (property_bag::dup (pbag));

        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, _dominate: i32) {
        let nx = self.x.get ().normalize (draw_ctx);
        let ny = self.y.get ().normalize (draw_ctx);
        let nw = self.w.get ().normalize (draw_ctx);
        let nh = self.h.get ().normalize (draw_ctx);

        let state = drawing_ctx::get_current_state (draw_ctx);
        let do_clip = !drawing_ctx::state_is_overflow (state) && node.get_parent ().is_some ();

        draw_in_viewport(nx, ny, nw, nh,
                         ClipMode::ClipToViewport,
                         do_clip,
                         self.vbox.get(),
                         self.preserve_aspect_ratio.get(),
                         drawing_ctx::get_current_state_affine(draw_ctx),
                         draw_ctx,
                         || {
                             drawing_ctx::state_push(draw_ctx);
                             node.draw_children(draw_ctx, -1); // dominate==-1 so it won't reinherit or push a layer
                             drawing_ctx::state_pop(draw_ctx);
                         });
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

        self.x.set (property_bag::parse_or_default (pbag, "x", LengthDir::Horizontal, None)?);
        self.y.set (property_bag::parse_or_default (pbag, "y", LengthDir::Vertical, None)?);

        self.w.set (property_bag::parse_or_none (pbag, "width", LengthDir::Horizontal,
                                                 Some(RsvgLength::check_nonnegative))?);

        self.h.set (property_bag::parse_or_none (pbag, "height", LengthDir::Vertical,
                                                 Some(RsvgLength::check_nonnegative))?);

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
        if double_equals (nw, 0.0) || double_equals (nh, 0.0) {
            drawing_ctx::release_node (draw_ctx, raw_child);
            return;
        }
        
        drawing_ctx::state_reinherit_top (draw_ctx, node.get_state (), dominate);

        let state = drawing_ctx::get_current_state (draw_ctx);

        if child.get_type () != NodeType::Symbol {
            let mut affine = drawing_ctx::get_current_state_affine (draw_ctx);
            affine.translate (nx, ny);
            drawing_ctx::set_current_state_affine (draw_ctx, affine);

            drawing_ctx::push_discrete_layer (draw_ctx);

            let boxed_child = box_node (child.clone ());
            drawing_ctx::draw_node_from_stack (draw_ctx, boxed_child, 1);
            rsvg_node_unref (boxed_child);

            drawing_ctx::release_node (draw_ctx, raw_child);
            drawing_ctx::pop_discrete_layer (draw_ctx);
        } else {
            child.with_impl (|symbol: &NodeSymbol| {
                let do_clip = !drawing_ctx::state_is_overflow (state)
                    || (!drawing_ctx::state_has_overflow (state)
                        && drawing_ctx::state_is_overflow (child.get_state ()));

                draw_in_viewport(nx, ny, nw, nh,
                                 ClipMode::ClipToVbox,
                                 do_clip,
                                 symbol.vbox.get(),
                                 symbol.preserve_aspect_ratio.get(),
                                 drawing_ctx::get_current_state_affine(draw_ctx),
                                 draw_ctx,
                                 || {
                                     drawing_ctx::state_push(draw_ctx);
                                     child.draw_children(draw_ctx, 1);
                                     drawing_ctx::state_pop(draw_ctx);
                                 });
            });

            drawing_ctx::release_node (draw_ctx, raw_child);
        }
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
        self.preserve_aspect_ratio.set (property_bag::parse_or_default (pbag, "preserveAspectRatio", (), None)?);
        self.vbox.set (property_bag::parse_or_none (pbag, "viewBox", (), None)?);

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
