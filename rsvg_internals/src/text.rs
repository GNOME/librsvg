use libc;
use glib::translate::*;
use glib_sys;
use std::cell::RefCell;

use attributes::Attribute;
use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode, boxed_node_new};
use property_bag::PropertyBag;

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
    link: RefCell<Option<String>>
}

impl NodeTRef {
    fn new() -> NodeTRef {
        NodeTRef {
            link: RefCell::new(Default::default())
        }
    }

    fn measure(&self, draw_ctx: *const RsvgDrawingCtx, length: &mut f64) -> bool {
        let l = self.link.borrow();

        if l.is_none() {
            return false;
        }

        let done = if let Some(acquired) =
            drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap())
        {
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

        if let Some(acquired) =
            drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap())
        {
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

fn measure_children(node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, length: &mut f64, textonly: bool) -> bool {
    let done = unsafe { rsvg_text_measure_children(node as *const RsvgNode, draw_ctx, length, textonly.to_glib()) };
    from_glib(done)
}

fn render_children(node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, x: &mut f64, y: &mut f64, textonly: bool) {
    unsafe { rsvg_text_render_children(node as *const RsvgNode, draw_ctx, x, y, textonly.to_glib()) };
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
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

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
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert!(!raw_x.is_null());
    assert!(!raw_y.is_null());
    let x: &mut f64 = unsafe { &mut *raw_x };
    let y: &mut f64 = unsafe { &mut *raw_y };

    node.with_impl(|tref: &NodeTRef| {
        tref.render(draw_ctx, x, y);
    });
}
