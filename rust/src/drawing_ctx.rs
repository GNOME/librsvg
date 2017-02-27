extern crate glib;
extern crate cairo;
extern crate cairo_sys;
extern crate libc;

use self::glib::translate::*;

use node::RsvgNode;
use node::NodeType;
use path_builder::RsvgPathBuilder;
use state::RsvgState;

pub enum RsvgDrawingCtx {}

extern "C" {
    fn rsvg_drawing_ctx_get_dpi (draw_ctx: *const RsvgDrawingCtx,
                                 out_dpi_x: *mut f64,
                                 out_dpi_y: *mut f64);

    fn rsvg_drawing_ctx_get_normalized_font_size (draw_ctx: *const RsvgDrawingCtx) -> f64;

    fn rsvg_drawing_ctx_get_view_box_size (draw_ctx: *const RsvgDrawingCtx,
                                           out_x: *mut f64,
                                           out_y: *mut f64);

    fn rsvg_drawing_ctx_push_view_box (draw_ctx: *const RsvgDrawingCtx,
                                       width:     f64,
                                       height:    f64);

    fn rsvg_drawing_ctx_pop_view_box (draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_drawing_ctx_acquire_node (draw_ctx: *const RsvgDrawingCtx,
                                      url:      *const libc::c_char) -> *mut RsvgNode;

    fn rsvg_drawing_ctx_acquire_node_of_type (draw_ctx:  *const RsvgDrawingCtx,
                                              url:       *const libc::c_char,
                                              node_type: NodeType) -> *mut RsvgNode;

    fn rsvg_drawing_ctx_release_node (draw_ctx: *const RsvgDrawingCtx,
                                      node:     *mut RsvgNode);

    fn rsvg_drawing_ctx_get_current_state_affine (draw_ctx: *const RsvgDrawingCtx) -> cairo::Matrix;

    fn rsvg_drawing_ctx_set_current_state_affine (draw_ctx: *const RsvgDrawingCtx,
                                                  affine:   *const cairo::Matrix);

    fn rsvg_drawing_ctx_add_clipping_rect (draw_ctx: *const RsvgDrawingCtx,
                                           x: f64,
                                           y: f64,
                                           w: f64,
                                           h: f64);

    fn rsvg_current_state (draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState;
    fn rsvg_state_new () -> *mut RsvgState;
    fn rsvg_state_reinit (state: *mut RsvgState);
    fn rsvg_state_reconstruct (state: *mut RsvgState, node: *const RsvgNode);
    fn rsvg_state_is_overflow (state: *const RsvgState) -> bool;

    fn rsvg_state_push (draw_ctx: *const RsvgDrawingCtx);
    fn rsvg_state_pop (draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_state_reinherit_top (draw_ctx: *const RsvgDrawingCtx,
                                 state: *mut RsvgState,
                                 dominate: libc::c_int);

    fn rsvg_push_discrete_layer (draw_ctx: *const RsvgDrawingCtx);
    fn rsvg_pop_discrete_layer (draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_render_path_builder (draw_ctx: *const RsvgDrawingCtx,
                                 builder: *const RsvgPathBuilder);

    fn rsvg_cairo_get_cairo_context (draw_ctx: *const RsvgDrawingCtx) -> *mut cairo_sys::cairo_t;
    fn rsvg_cairo_set_cairo_context (draw_ctx: *const RsvgDrawingCtx, cr: *const cairo_sys::cairo_t);

    fn _rsvg_node_draw_children (node: *const RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: libc::c_int);
}

pub fn get_dpi (draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut dpi_x: f64 = 0.0;
    let mut dpi_y: f64 = 0.0;

    unsafe { rsvg_drawing_ctx_get_dpi (draw_ctx, &mut dpi_x, &mut dpi_y); }

    (dpi_x, dpi_y)
}


pub fn get_normalized_font_size (draw_ctx: *const RsvgDrawingCtx) -> f64 {
    unsafe { rsvg_drawing_ctx_get_normalized_font_size (draw_ctx) }
}

pub fn get_view_box_size (draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe { rsvg_drawing_ctx_get_view_box_size (draw_ctx, &mut w, &mut h); }

    (w, h)
}

pub fn push_view_box (draw_ctx: *const RsvgDrawingCtx,
                      width:     f64,
                      height:    f64)
{
    unsafe { rsvg_drawing_ctx_push_view_box (draw_ctx, width, height); }
}

pub fn pop_view_box (draw_ctx: *const RsvgDrawingCtx) {
    unsafe { rsvg_drawing_ctx_pop_view_box (draw_ctx); }
}

pub fn acquire_node (draw_ctx: *const RsvgDrawingCtx,
                     url:      &str) -> *mut RsvgNode {
    unsafe { rsvg_drawing_ctx_acquire_node (draw_ctx, str::to_glib_none (url).0) }
}

pub fn acquire_node_of_type (draw_ctx:  *const RsvgDrawingCtx,
                             url:       &str,
                             node_type: NodeType) -> *mut RsvgNode {
    unsafe { rsvg_drawing_ctx_acquire_node_of_type (draw_ctx, str::to_glib_none (url).0, node_type) }
}

pub fn release_node (draw_ctx: *const RsvgDrawingCtx,
                     node:     *mut RsvgNode) {
    unsafe { rsvg_drawing_ctx_release_node (draw_ctx, node); }
}

pub fn state_reinherit_top (draw_ctx: *const RsvgDrawingCtx,
                            state: *mut RsvgState,
                            dominate: i32) {
    unsafe { rsvg_state_reinherit_top (draw_ctx, state, dominate); }
}

pub fn push_discrete_layer (draw_ctx: *const RsvgDrawingCtx) {
    unsafe { rsvg_push_discrete_layer (draw_ctx); }
}

pub fn pop_discrete_layer (draw_ctx: *const RsvgDrawingCtx) {
    unsafe { rsvg_pop_discrete_layer (draw_ctx); }
}

pub fn render_path_builder (draw_ctx: *const RsvgDrawingCtx,
                            builder: &RsvgPathBuilder) {
    unsafe { rsvg_render_path_builder (draw_ctx, builder); }
}

pub fn get_cairo_context (draw_ctx: *const RsvgDrawingCtx) -> cairo::Context {
    unsafe {
        let raw_cr = rsvg_cairo_get_cairo_context (draw_ctx);

        let cr = cairo::Context::from_glib_none (raw_cr);

        cr
    }
}

pub fn set_cairo_context (draw_ctx: *const RsvgDrawingCtx, cr: &cairo::Context) {
    unsafe {
        let raw_cr = cr.to_glib_none ().0;

        rsvg_cairo_set_cairo_context (draw_ctx, raw_cr);
    }
}

pub fn get_current_state_affine (draw_ctx: *const RsvgDrawingCtx) -> cairo::Matrix {
    unsafe {
        rsvg_drawing_ctx_get_current_state_affine (draw_ctx)
    }
}

pub fn set_current_state_affine (draw_ctx: *const RsvgDrawingCtx, affine: cairo::Matrix) {
    unsafe {
        rsvg_drawing_ctx_set_current_state_affine (draw_ctx, &affine);
    }
}

pub fn add_clipping_rect (draw_ctx: *const RsvgDrawingCtx,
                          x: f64,
                          y: f64,
                          w: f64,
                          h: f64) {
    unsafe { rsvg_drawing_ctx_add_clipping_rect (draw_ctx, x, y, w, h); }
}

pub fn get_current_state (draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState {
    unsafe { rsvg_current_state (draw_ctx) }
}

pub fn state_new () -> *mut RsvgState {
    unsafe { rsvg_state_new () }
}

pub fn state_reinit (state: *mut RsvgState) {
    unsafe { rsvg_state_reinit (state); }
}

pub fn state_reconstruct (state: *mut RsvgState, node: *const RsvgNode) {
    unsafe { rsvg_state_reconstruct (state, node); }
}

pub fn state_is_overflow (state: *const RsvgState) -> bool {
    unsafe { rsvg_state_is_overflow (state) }
}

pub fn state_push (draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_state_push (draw_ctx);
    }
}

pub fn state_pop (draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_state_pop (draw_ctx);
    }
}

pub fn node_draw_children (draw_ctx: *const RsvgDrawingCtx, c_node: *const RsvgNode, dominate: libc::c_int) {
    unsafe {
        _rsvg_node_draw_children (c_node, draw_ctx, dominate);
    }
}
