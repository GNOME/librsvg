extern crate glib;
extern crate cairo;
extern crate libc;

use self::glib::translate::*;

use std::cell::Cell;
use std::ptr;

use aspect_ratio::*;
use drawing_ctx::RsvgDrawingCtx;
use drawing_ctx;
use handle::RsvgHandle;
use length::*;
use node::*;
use property_bag;
use property_bag::*;
use util::*;
use viewbox::*;

use self::cairo::MatrixTrait;


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
    vbox:                  Cell<RsvgViewBox>,
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
            vbox:                  Cell::new (RsvgViewBox::default ()),
            atts:                  Cell::new (ptr::null_mut ())
        }
    }
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

        self.w.set (property_bag::length_or_value (pbag, "width", LengthDir::Horizontal, "100%")?);
        self.h.set (property_bag::length_or_value (pbag, "height", LengthDir::Vertical, "100%")?);

        self.vbox.set (property_bag::parse_or_default (pbag, "viewBox")?);

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

        drawing_ctx::state_reinherit_top (draw_ctx, node.get_state (), dominate);

        let state = drawing_ctx::get_current_state (draw_ctx);

        let affine_old = drawing_ctx::get_current_state_affine (draw_ctx);

        let vbox = self.vbox.get ();

        if vbox.active {
            // viewBox width==0 or height==0 disables rendering of the element
            // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
            if double_equals (vbox.rect.width, 0.0) || double_equals (vbox.rect.height, 0.0) {
                return;
            }

            let (x, y, w, h) = self.preserve_aspect_ratio.get ().compute (vbox.rect.width, vbox.rect.height,
                                                                          nx, ny, nw, nh);

            let mut affine = affine_old;
            affine.translate (x, y);
            affine.scale (w / vbox.rect.width, h / vbox.rect.height);
            affine.translate (-vbox.rect.x, -vbox.rect.y);
            drawing_ctx::set_current_state_affine (draw_ctx, affine);

            drawing_ctx::push_view_box (draw_ctx, vbox.rect.width, vbox.rect.height);
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

    let mut vbox = RsvgViewBox::default ();

    node.with_impl (|svg: &NodeSvg| {
        vbox = svg.vbox.get ();
    });

    vbox
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

            unsafe { rsvg_parse_style_attrs (handle,
                                             raw_node,
                                             str::to_glib_none ("svg").0,
                                             class.map_or (ptr::null (), |s| String::to_glib_none (&s).0),
                                             id.map_or (ptr::null (), |s| String::to_glib_none (&s).0),
                                             pbag); }
        }
    });
}
