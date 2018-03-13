use cairo;
use cairo_sys;
use glib::translate::*;
use glib_sys;
use libc;
use pango;
use pango_sys;

use color::*;
use error::*;
use node::NodeType;
use node::RsvgNode;
use opacity::*;
use path_builder::RsvgPathBuilder;
use state::RsvgState;

pub enum RsvgDrawingCtx {}

extern "C" {
    fn rsvg_drawing_ctx_get_dpi(
        draw_ctx: *const RsvgDrawingCtx,
        out_dpi_x: *mut f64,
        out_dpi_y: *mut f64,
    );

    fn rsvg_drawing_ctx_get_normalized_font_size(draw_ctx: *const RsvgDrawingCtx) -> f64;

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

    fn rsvg_drawing_ctx_get_pango_context(
        draw_ctx: *const RsvgDrawingCtx,
    ) -> *mut pango_sys::PangoContext;

    fn rsvg_drawing_ctx_add_clipping_rect(
        draw_ctx: *const RsvgDrawingCtx,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    );

    fn rsvg_drawing_ctx_draw_node_from_stack(
        draw_ctx: *const RsvgDrawingCtx,
        node: *const RsvgNode,
        dominate: i32,
    );

    fn rsvg_current_state(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState;
    fn rsvg_state_new() -> *mut RsvgState;
    fn rsvg_state_free(state: *mut RsvgState);
    fn rsvg_state_reinit(state: *mut RsvgState);
    fn rsvg_state_reconstruct(state: *mut RsvgState, node: *const RsvgNode);
    fn rsvg_state_is_overflow(state: *const RsvgState) -> glib_sys::gboolean;
    fn rsvg_state_has_overflow(state: *const RsvgState) -> glib_sys::gboolean;
    fn rsvg_state_get_cond_true(state: *const RsvgState) -> glib_sys::gboolean;
    fn rsvg_state_set_cond_true(state: *const RsvgState, cond_true: glib_sys::gboolean);
    fn rsvg_state_get_stop_color(state: *const RsvgState) -> *const ColorSpec;
    fn rsvg_state_get_stop_opacity(state: *const RsvgState) -> *const OpacitySpec;
    fn rsvg_state_get_current_color(state: *const RsvgState) -> u32;

    fn rsvg_state_push(draw_ctx: *const RsvgDrawingCtx);
    fn rsvg_state_pop(draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_state_reinherit_top(
        draw_ctx: *const RsvgDrawingCtx,
        state: *mut RsvgState,
        dominate: libc::c_int,
    );

    fn rsvg_push_discrete_layer(draw_ctx: *const RsvgDrawingCtx);
    fn rsvg_pop_discrete_layer(draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_render_path_builder(draw_ctx: *const RsvgDrawingCtx, builder: *const RsvgPathBuilder);
    fn rsvg_render_surface(
        draw_ctx: *const RsvgDrawingCtx,
        surface: *const cairo_sys::cairo_surface_t,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    );

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
    unsafe { rsvg_drawing_ctx_get_normalized_font_size(draw_ctx) }
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

pub fn acquire_node(draw_ctx: *const RsvgDrawingCtx, url: &str) -> *mut RsvgNode {
    unsafe { rsvg_drawing_ctx_acquire_node(draw_ctx, str::to_glib_none(url).0) }
}

pub fn acquire_node_of_type(
    draw_ctx: *const RsvgDrawingCtx,
    url: &str,
    node_type: NodeType,
) -> *mut RsvgNode {
    unsafe { rsvg_drawing_ctx_acquire_node_of_type(draw_ctx, str::to_glib_none(url).0, node_type) }
}

pub fn release_node(draw_ctx: *const RsvgDrawingCtx, node: *mut RsvgNode) {
    unsafe {
        rsvg_drawing_ctx_release_node(draw_ctx, node);
    }
}

pub fn state_reinherit_top(draw_ctx: *const RsvgDrawingCtx, state: *mut RsvgState, dominate: i32) {
    unsafe {
        rsvg_state_reinherit_top(draw_ctx, state, dominate);
    }
}

pub fn push_discrete_layer(draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_push_discrete_layer(draw_ctx);
    }
}

pub fn pop_discrete_layer(draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_pop_discrete_layer(draw_ctx);
    }
}

pub fn render_path_builder(draw_ctx: *const RsvgDrawingCtx, builder: &RsvgPathBuilder) {
    unsafe {
        rsvg_render_path_builder(draw_ctx, builder);
    }
}

pub fn render_surface(
    draw_ctx: *const RsvgDrawingCtx,
    surface: &cairo::ImageSurface,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    unsafe {
        rsvg_render_surface(draw_ctx, surface.to_raw_none(), x, y, w, h);
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

pub fn get_pango_context(draw_ctx: *const RsvgDrawingCtx) -> pango::Context {
    unsafe { from_glib_full(rsvg_drawing_ctx_get_pango_context(draw_ctx)) }
}

pub fn add_clipping_rect(draw_ctx: *const RsvgDrawingCtx, x: f64, y: f64, w: f64, h: f64) {
    unsafe {
        rsvg_drawing_ctx_add_clipping_rect(draw_ctx, x, y, w, h);
    }
}

pub fn draw_node_from_stack(draw_ctx: *const RsvgDrawingCtx, node: *const RsvgNode, dominate: i32) {
    unsafe {
        rsvg_drawing_ctx_draw_node_from_stack(draw_ctx, node, dominate);
    }
}

pub fn get_current_state(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState {
    unsafe { rsvg_current_state(draw_ctx) }
}

pub fn state_new() -> *mut RsvgState {
    unsafe { rsvg_state_new() }
}

pub fn state_free(state: *mut RsvgState) {
    unsafe {
        rsvg_state_free(state);
    }
}

pub fn state_reinit(state: *mut RsvgState) {
    unsafe {
        rsvg_state_reinit(state);
    }
}

pub fn state_reconstruct(state: *mut RsvgState, node: *const RsvgNode) {
    unsafe {
        rsvg_state_reconstruct(state, node);
    }
}

pub fn state_is_overflow(state: *const RsvgState) -> bool {
    unsafe { from_glib(rsvg_state_is_overflow(state)) }
}

pub fn state_has_overflow(state: *const RsvgState) -> bool {
    unsafe { from_glib(rsvg_state_has_overflow(state)) }
}

pub fn state_get_cond_true(state: *const RsvgState) -> bool {
    unsafe { from_glib(rsvg_state_get_cond_true(state)) }
}

pub fn state_set_cond_true(state: *const RsvgState, cond_true: bool) {
    unsafe {
        rsvg_state_set_cond_true(state, cond_true.to_glib());
    }
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

pub fn state_get_stop_color(state: *const RsvgState) -> Result<Option<Color>, AttributeError> {
    unsafe {
        let spec_ptr = rsvg_state_get_stop_color(state);

        if spec_ptr.is_null() {
            Ok(None)
        } else {
            Color::from_color_spec(&*spec_ptr).map(Some)
        }
    }
}

pub fn state_get_stop_opacity(state: *const RsvgState) -> Result<Option<Opacity>, AttributeError> {
    unsafe {
        let opacity_ptr = rsvg_state_get_stop_opacity(state);

        if opacity_ptr.is_null() {
            Ok(None)
        } else {
            Opacity::from_opacity_spec(&*opacity_ptr).map(Some)
        }
    }
}

pub fn state_get_current_color(state: *const RsvgState) -> Color {
    let argb = unsafe { rsvg_state_get_current_color(state) };

    Color::from(argb)
}

pub struct AcquiredNode(*const RsvgDrawingCtx, *mut RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        release_node(self.0, self.1);
    }
}

impl AcquiredNode {
    pub fn get(&self) -> RsvgNode {
        unsafe { (*self.1).clone() }
    }
}
