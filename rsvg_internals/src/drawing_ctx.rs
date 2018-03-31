use cairo;
use cairo_sys;
use glib::translate::*;
use glib_sys;
use libc;
use pango;
use pango_sys;

use bbox::RsvgBbox;
use length::LengthUnit;
use node::NodeType;
use node::RsvgNode;
use state::{self, FontSize, RsvgState};

pub enum RsvgDrawingCtx {}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_drawing_ctx_get_dpi(
        draw_ctx: *const RsvgDrawingCtx,
        out_dpi_x: *mut f64,
        out_dpi_y: *mut f64,
    );

    fn rsvg_drawing_ctx_get_view_box_size(
        draw_ctx: *const RsvgDrawingCtx,
        out_x: *mut f64,
        out_y: *mut f64,
    );

    fn rsvg_drawing_ctx_push_view_box(draw_ctx: *const RsvgDrawingCtx, width: f64, height: f64);

    fn rsvg_drawing_ctx_pop_view_box(draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_drawing_ctx_acquire_node(
        draw_ctx: *const RsvgDrawingCtx,
        url: *const libc::c_char,
    ) -> *mut RsvgNode;

    fn rsvg_drawing_ctx_acquire_node_of_type(
        draw_ctx: *const RsvgDrawingCtx,
        url: *const libc::c_char,
        node_type: NodeType,
    ) -> *mut RsvgNode;

    fn rsvg_drawing_ctx_release_node(draw_ctx: *const RsvgDrawingCtx, node: *mut RsvgNode);

    fn rsvg_drawing_ctx_get_current_state_affine(draw_ctx: *const RsvgDrawingCtx) -> cairo::Matrix;

    fn rsvg_drawing_ctx_set_current_state_affine(
        draw_ctx: *const RsvgDrawingCtx,
        affine: *const cairo::Matrix,
    );

    fn rsvg_drawing_ctx_set_affine_on_cr(
        draw_ctx: *const RsvgDrawingCtx,
        cr: *mut cairo_sys::cairo_t,
        affine: *const cairo::Matrix,
    );

    fn rsvg_drawing_ctx_get_pango_context(
        draw_ctx: *const RsvgDrawingCtx,
    ) -> *mut pango_sys::PangoContext;

    fn rsvg_drawing_ctx_insert_bbox(draw_ctx: *const RsvgDrawingCtx, bbox: *const RsvgBbox);

    fn rsvg_drawing_ctx_draw_node_from_stack(
        draw_ctx: *const RsvgDrawingCtx,
        node: *const RsvgNode,
        dominate: i32,
        clipping: glib_sys::gboolean,
    );

    fn rsvg_current_state(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState;

    fn rsvg_state_push(draw_ctx: *const RsvgDrawingCtx);
    fn rsvg_state_pop(draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_state_reinherit_top(
        draw_ctx: *const RsvgDrawingCtx,
        state: *mut RsvgState,
        dominate: libc::c_int,
    );

    fn rsvg_push_discrete_layer(draw_ctx: *const RsvgDrawingCtx, clipping: glib_sys::gboolean);
    fn rsvg_pop_discrete_layer(draw_ctx: *const RsvgDrawingCtx, clipping: glib_sys::gboolean);

    fn rsvg_cairo_get_cairo_context(draw_ctx: *const RsvgDrawingCtx) -> *mut cairo_sys::cairo_t;
    fn rsvg_cairo_set_cairo_context(draw_ctx: *const RsvgDrawingCtx, cr: *const cairo_sys::cairo_t);
}

pub fn get_dpi(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut dpi_x: f64 = 0.0;
    let mut dpi_y: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_dpi(draw_ctx, &mut dpi_x, &mut dpi_y);
    }

    (dpi_x, dpi_y)
}

pub fn get_normalized_font_size(draw_ctx: *const RsvgDrawingCtx) -> f64 {
    normalize_font_size(draw_ctx, get_current_state(draw_ctx))
}

// Recursive evaluation of all parent elements regarding absolute font size
pub fn normalize_font_size(draw_ctx: *const RsvgDrawingCtx, state: *const RsvgState) -> f64 {
    let font_size = state::get_state_rust(state)
        .font_size
        .as_ref()
        .map_or_else(|| FontSize::default().0, |fs| fs.0);

    match font_size.unit {
        LengthUnit::Percent | LengthUnit::FontEm | LengthUnit::FontEx => {
            parent_font_size(draw_ctx, state) * font_size.length
        }
        LengthUnit::RelativeLarger => parent_font_size(draw_ctx, state) * 1.2f64,
        LengthUnit::RelativeSmaller => parent_font_size(draw_ctx, state) / 1.2f64,

        _ => font_size.normalize(draw_ctx),
    }
}

fn parent_font_size(draw_ctx: *const RsvgDrawingCtx, state: *const RsvgState) -> f64 {
    state::parent(state).map_or(12f64, |p| normalize_font_size(draw_ctx, p))
}

pub fn get_view_box_size(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_view_box_size(draw_ctx, &mut w, &mut h);
    }

    (w, h)
}

pub fn push_view_box(draw_ctx: *const RsvgDrawingCtx, width: f64, height: f64) {
    unsafe {
        rsvg_drawing_ctx_push_view_box(draw_ctx, width, height);
    }
}

pub fn pop_view_box(draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_drawing_ctx_pop_view_box(draw_ctx);
    }
}

pub fn get_acquired_node(draw_ctx: *const RsvgDrawingCtx, url: &str) -> Option<AcquiredNode> {
    let raw_node = unsafe { rsvg_drawing_ctx_acquire_node(draw_ctx, str::to_glib_none(url).0) };

    if raw_node.is_null() {
        None
    } else {
        Some(AcquiredNode(draw_ctx, raw_node))
    }
}

pub fn get_acquired_node_of_type(
    draw_ctx: *const RsvgDrawingCtx,
    url: &str,
    node_type: NodeType,
) -> Option<AcquiredNode> {
    let raw_node = unsafe {
        rsvg_drawing_ctx_acquire_node_of_type(draw_ctx, str::to_glib_none(url).0, node_type)
    };

    if raw_node.is_null() {
        None
    } else {
        Some(AcquiredNode(draw_ctx, raw_node))
    }
}

pub fn state_reinherit_top(draw_ctx: *const RsvgDrawingCtx, state: *mut RsvgState, dominate: i32) {
    unsafe {
        rsvg_state_reinherit_top(draw_ctx, state, dominate);
    }
}

pub fn push_discrete_layer(draw_ctx: *const RsvgDrawingCtx, clipping: bool) {
    unsafe {
        rsvg_push_discrete_layer(draw_ctx, clipping.to_glib());
    }
}

pub fn pop_discrete_layer(draw_ctx: *const RsvgDrawingCtx, clipping: bool) {
    unsafe {
        rsvg_pop_discrete_layer(draw_ctx, clipping.to_glib());
    }
}

pub fn get_cairo_context(draw_ctx: *const RsvgDrawingCtx) -> cairo::Context {
    unsafe {
        let raw_cr = rsvg_cairo_get_cairo_context(draw_ctx);

        cairo::Context::from_glib_none(raw_cr)
    }
}

pub fn set_cairo_context(draw_ctx: *const RsvgDrawingCtx, cr: &cairo::Context) {
    unsafe {
        let raw_cr = cr.to_glib_none().0;

        rsvg_cairo_set_cairo_context(draw_ctx, raw_cr);
    }
}

pub fn get_current_state_affine(draw_ctx: *const RsvgDrawingCtx) -> cairo::Matrix {
    unsafe { rsvg_drawing_ctx_get_current_state_affine(draw_ctx) }
}

pub fn set_current_state_affine(draw_ctx: *const RsvgDrawingCtx, affine: cairo::Matrix) {
    unsafe {
        rsvg_drawing_ctx_set_current_state_affine(draw_ctx, &affine);
    }
}

pub fn set_affine_on_cr(
    draw_ctx: *const RsvgDrawingCtx,
    cr: &cairo::Context,
    affine: &cairo::Matrix,
) {
    unsafe {
        rsvg_drawing_ctx_set_affine_on_cr(
            draw_ctx,
            cr.to_glib_none().0,
            affine as *const cairo::Matrix,
        );
    }
}

pub fn get_pango_context(draw_ctx: *const RsvgDrawingCtx) -> pango::Context {
    unsafe { from_glib_full(rsvg_drawing_ctx_get_pango_context(draw_ctx)) }
}

pub fn insert_bbox(draw_ctx: *const RsvgDrawingCtx, bbox: &RsvgBbox) {
    unsafe {
        rsvg_drawing_ctx_insert_bbox(draw_ctx, bbox as *const _);
    }
}

pub fn draw_node_from_stack(
    draw_ctx: *const RsvgDrawingCtx,
    node: *const RsvgNode,
    dominate: i32,
    clipping: bool,
) {
    unsafe {
        rsvg_drawing_ctx_draw_node_from_stack(draw_ctx, node, dominate, clipping.to_glib());
    }
}

pub fn get_current_state(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState {
    unsafe { rsvg_current_state(draw_ctx) }
}

pub fn state_push(draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_state_push(draw_ctx);
    }
}

pub fn state_pop(draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_state_pop(draw_ctx);
    }
}

pub struct AcquiredNode(*const RsvgDrawingCtx, *mut RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        unsafe {
            rsvg_drawing_ctx_release_node(self.0, self.1);
        }
    }
}

impl AcquiredNode {
    pub fn get(&self) -> RsvgNode {
        unsafe { (*self.1).clone() }
    }
}
