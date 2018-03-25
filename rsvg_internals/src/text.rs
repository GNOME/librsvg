use libc;
use glib::translate::*;
use glib_sys;
use std::cell::{Cell, RefCell};

use attributes::Attribute;
use chars;
use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use length::*;
use node::{boxed_node_new, NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;
use state::{self, TextAnchor};

extern "C" {
    fn rsvg_text_measure_children(
        raw_node: *const RsvgNode,
        draw_ctx: *const RsvgDrawingCtx,
        raw_length: *mut libc::c_double,
        usetextonly: glib_sys::gboolean,
    ) -> glib_sys::gboolean;

    fn rsvg_text_render_children(
        raw_node: *const RsvgNode,
        draw_ctx: *const RsvgDrawingCtx,
        raw_x: *mut libc::c_double,
        raw_y: *mut libc::c_double,
        usetextonly: glib_sys::gboolean,
    );
}

struct NodeTRef {
    link: RefCell<Option<String>>,
}

impl NodeTRef {
    fn new() -> NodeTRef {
        NodeTRef {
            link: RefCell::new(Default::default()),
        }
    }

    fn measure(&self, draw_ctx: *const RsvgDrawingCtx, length: &mut f64) -> bool {
        let l = self.link.borrow();

        if l.is_none() {
            return false;
        }

        let done =
            if let Some(acquired) = drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap()) {
                let c = acquired.get();
                measure_children(&c, draw_ctx, length, true)
            } else {
                false
            };

        done
    }

    fn render(&self, draw_ctx: *const RsvgDrawingCtx, x: &mut f64, y: &mut f64) {
        let l = self.link.borrow();

        if l.is_none() {
            return;
        }

        if let Some(acquired) = drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap()) {
            let c = acquired.get();
            render_children(&c, draw_ctx, x, y, true)
        }
    }
}

impl NodeTrait for NodeTRef {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

struct NodeTSpan {
    x: Cell<Option<RsvgLength>>,
    y: Cell<Option<RsvgLength>>,
    dx: Cell<RsvgLength>,
    dy: Cell<RsvgLength>,
}

impl NodeTSpan {
    fn new() -> NodeTSpan {
        NodeTSpan {
            x: Cell::new(Default::default()),
            y: Cell::new(Default::default()),
            dx: Cell::new(RsvgLength::default()),
            dy: Cell::new(RsvgLength::default()),
        }
    }

    fn measure(
        &self,
        node: &RsvgNode,
        draw_ctx: *const RsvgDrawingCtx,
        length: &mut f64,
        usetextonly: bool,
    ) -> bool {
        if self.x.get().is_some() || self.y.get().is_some() {
            return true;
        }

        let state = drawing_ctx::get_current_state(draw_ctx);
        let gravity = state::get_text_gravity(state);
        if chars::gravity_is_vertical(gravity) {
            *length += self.dy.get().normalize(draw_ctx);
        } else {
            *length += self.dx.get().normalize(draw_ctx);
        }

        measure_children(node, draw_ctx, length, usetextonly)
    }

    fn render(
        &self,
        node: &RsvgNode,
        draw_ctx: *const RsvgDrawingCtx,
        x: &mut f64,
        y: &mut f64,
        usetextonly: bool,
    ) {
        drawing_ctx::state_push(draw_ctx);
        drawing_ctx::state_reinherit_top(draw_ctx, node.get_state(), 0);

        let mut dx = self.dx.get().normalize(draw_ctx);
        let mut dy = self.dy.get().normalize(draw_ctx);

        let state = drawing_ctx::get_current_state(draw_ctx);
        let anchor = state::get_state_rust(state).text_anchor;
        let gravity = state::get_text_gravity(state);

        let offset = anchor_offset(node, draw_ctx, anchor, usetextonly);

        if let Some(self_x) = self.x.get() {
            *x = self_x.normalize(draw_ctx);
            if !chars::gravity_is_vertical(gravity) {
                *x -= offset;
                dx = match anchor {
                    TextAnchor::Start => dx,
                    TextAnchor::Middle => dx / 2f64,
                    _ => 0f64,
                }
            }
        }
        *x += dx;

        if let Some(self_y) = self.y.get() {
            *y = self_y.normalize(draw_ctx);
            if chars::gravity_is_vertical(gravity) {
                *y -= offset;
                dy = match anchor {
                    TextAnchor::Start => dy,
                    TextAnchor::Middle => dy / 2f64,
                    _ => 0f64,
                }
            }
        }
        *y += dy;

        render_children(node, draw_ctx, x, y, usetextonly);

        drawing_ctx::state_pop(draw_ctx);
    }
}

impl NodeTrait for NodeTSpan {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x
                    .set(parse("x", value, LengthDir::Horizontal, None).map(Some)?),
                Attribute::Y => self.y
                    .set(parse("y", value, LengthDir::Vertical, None).map(Some)?),
                Attribute::Dx => self.dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

fn anchor_offset(
    node: &RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    anchor: TextAnchor,
    textonly: bool,
) -> f64 {
    let mut offset = 0f64;

    match anchor {
        TextAnchor::Start => {}
        TextAnchor::Middle => {
            measure_children(node, draw_ctx, &mut offset, textonly);
            offset /= 2f64;
        }
        _ => {
            measure_children(node, draw_ctx, &mut offset, textonly);
        }
    }

    offset
}

fn measure_children(
    node: &RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let done = unsafe {
        rsvg_text_measure_children(
            node as *const RsvgNode,
            draw_ctx,
            length,
            textonly.to_glib(),
        )
    };

    from_glib(done)
}

fn render_children(
    node: &RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
) {
    unsafe {
        rsvg_text_render_children(node as *const RsvgNode, draw_ctx, x, y, textonly.to_glib())
    };
}

#[no_mangle]
pub extern "C" fn rsvg_node_tref_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::TRef, raw_parent, Box::new(NodeTRef::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_tref_measure(
    raw_node: *const RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    raw_length: *mut libc::c_double,
) -> glib_sys::gboolean {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!raw_length.is_null());
    let length: &mut f64 = unsafe { &mut *raw_length };

    let mut done = false;
    node.with_impl(|tref: &NodeTRef| {
        done = tref.measure(draw_ctx, length);
    });

    done.to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_node_tref_render(
    raw_node: *const RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    raw_x: *mut libc::c_double,
    raw_y: *mut libc::c_double,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!raw_x.is_null());
    assert!(!raw_y.is_null());
    let x: &mut f64 = unsafe { &mut *raw_x };
    let y: &mut f64 = unsafe { &mut *raw_y };

    node.with_impl(|tref: &NodeTRef| {
        tref.render(draw_ctx, x, y);
    });
}

#[no_mangle]
pub extern "C" fn rsvg_node_tspan_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::TSpan, raw_parent, Box::new(NodeTSpan::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_tspan_measure(
    raw_node: *const RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    raw_length: *mut libc::c_double,
    usetextonly: glib_sys::gboolean,
) -> glib_sys::gboolean {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!raw_length.is_null());
    let length: &mut f64 = unsafe { &mut *raw_length };

    let textonly: bool = from_glib(usetextonly);

    let mut done = false;
    node.with_impl(|tspan: &NodeTSpan| {
        done = tspan.measure(&node, draw_ctx, length, textonly);
    });

    done.to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_node_tspan_render(
    raw_node: *const RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    raw_x: *mut libc::c_double,
    raw_y: *mut libc::c_double,
    usetextonly: glib_sys::gboolean,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!raw_x.is_null());
    assert!(!raw_y.is_null());
    let x: &mut f64 = unsafe { &mut *raw_x };
    let y: &mut f64 = unsafe { &mut *raw_y };

    let textonly: bool = from_glib(usetextonly);

    node.with_impl(|tspan: &NodeTSpan| {
        tspan.render(&node, draw_ctx, x, y, textonly);
    });
}
