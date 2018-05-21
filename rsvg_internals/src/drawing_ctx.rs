use cairo;
use cairo::MatrixTrait;
use cairo_sys;
use glib::translate::*;
use glib_sys;
use libc;
use pango::{self, FontMapExt};
use pango_cairo_sys;
use pangocairo;

use bbox::{BoundingBox, RsvgBbox};
use length::LengthUnit;
use node::NodeType;
use node::RsvgNode;
use rect::RectangleExt;
use state::{self, BaselineShift, FontSize, RsvgState, State};

pub enum RsvgDrawingCtx {}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_drawing_ctx_get_current_state(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState;
    fn rsvg_drawing_ctx_set_current_state(draw_ctx: *mut RsvgDrawingCtx, state: *mut RsvgState);

    fn rsvg_drawing_ctx_get_cairo_context(
        draw_ctx: *const RsvgDrawingCtx,
    ) -> *mut cairo_sys::cairo_t;

    fn rsvg_drawing_ctx_set_cairo_context(
        draw_ctx: *const RsvgDrawingCtx,
        cr: *const cairo_sys::cairo_t,
    );

    fn rsvg_drawing_ctx_is_cairo_context_nested(
        draw_ctx: *const RsvgDrawingCtx,
        cr: *const cairo_sys::cairo_t,
    ) -> glib_sys::gboolean;

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

    fn rsvg_drawing_ctx_get_offset(
        draw_ctx: *const RsvgDrawingCtx,
        out_x: *mut f64,
        out_y: *mut f64,
    );

    fn rsvg_drawing_ctx_insert_bbox(draw_ctx: *const RsvgDrawingCtx, bbox: *const RsvgBbox);

    fn rsvg_drawing_ctx_draw_node_from_stack(
        draw_ctx: *const RsvgDrawingCtx,
        node: *const RsvgNode,
        dominate: i32,
        clipping: glib_sys::gboolean,
    );

    fn rsvg_drawing_ctx_is_testing(draw_ctx: *const RsvgDrawingCtx) -> glib_sys::gboolean;

    fn rsvg_drawing_ctx_push_render_stack(draw_ctx: *const RsvgDrawingCtx);
    fn rsvg_drawing_ctx_pop_render_stack(draw_ctx: *const RsvgDrawingCtx);
}

pub fn get_cairo_context(draw_ctx: *const RsvgDrawingCtx) -> cairo::Context {
    unsafe {
        let raw_cr = rsvg_drawing_ctx_get_cairo_context(draw_ctx);

        cairo::Context::from_glib_none(raw_cr)
    }
}

pub fn set_cairo_context(draw_ctx: *const RsvgDrawingCtx, cr: &cairo::Context) {
    unsafe {
        let raw_cr = cr.to_glib_none().0;

        rsvg_drawing_ctx_set_cairo_context(draw_ctx, raw_cr);
    }
}

pub fn is_cairo_context_nested(draw_ctx: *const RsvgDrawingCtx, cr: &cairo::Context) -> bool {
    unsafe {
        let raw_cr = cr.to_glib_none().0;

        from_glib(rsvg_drawing_ctx_is_cairo_context_nested(draw_ctx, raw_cr))
    }
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
    normalize_font_size(draw_ctx, get_current_state(draw_ctx).unwrap())
}

pub fn get_accumulated_baseline_shift(draw_ctx: *const RsvgDrawingCtx) -> f64 {
    let mut shift = 0f64;

    let mut state = get_current_state(draw_ctx).unwrap();
    while let Some(parent) = state.parent() {
        if let Some(BaselineShift(ref s)) = state.baseline_shift {
            let parent_font_size = normalize_font_size(draw_ctx, parent);
            shift += s * parent_font_size;
        }
        state = parent;
    }

    shift
}

// Recursive evaluation of all parent elements regarding absolute font size
fn normalize_font_size(draw_ctx: *const RsvgDrawingCtx, state: &State) -> f64 {
    let font_size = state
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

fn parent_font_size(draw_ctx: *const RsvgDrawingCtx, state: &State) -> f64 {
    state
        .parent()
        .map_or(12f64, |p| normalize_font_size(draw_ctx, p))
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

// A function for modifying the top of the state stack depending on a
// flag given. If that flag is 0, style and transform will inherit
// normally. If that flag is 1, style will inherit normally with the
// exception that any value explicity set on the second last level
// will have a higher precedence than values set on the last level.
// If the flag equals two then the style will be overridden totally
// however the transform will be left as is. This is because of
// patterns which are not based on the context of their use and are
// rather based wholly on their own loading context. Other things
// may want to have this totally disabled, and a value of three will
// achieve this.
pub fn state_reinherit_top(draw_ctx: *const RsvgDrawingCtx, state: &State, dominate: i32) {
    let current = get_current_state_mut(draw_ctx).unwrap();

    match dominate {
        3 => unreachable!(),

        // This is a special domination mode for patterns, the transform
        // is simply left as is, wheras the style is totally overridden
        2 => current.force(state),

        dominate => {
            let parent_save = current.parent;
            current.clone_from(state);
            current.parent = parent_save;

            if let Some(parent) = current.parent() {
                if dominate == 0 {
                    current.reinherit(parent);
                } else {
                    current.dominate(parent);
                }

                current.affine = cairo::Matrix::multiply(&current.affine, &parent.affine);
            }
        }
    }
}

pub fn push_discrete_layer(draw_ctx: *const RsvgDrawingCtx, clipping: bool) {
    if !clipping {
        get_cairo_context(draw_ctx).save();

        unsafe {
            rsvg_drawing_ctx_push_render_stack(draw_ctx);
        }
    }
}

pub fn pop_discrete_layer(draw_ctx: *const RsvgDrawingCtx, clipping: bool) {
    if !clipping {
        unsafe {
            rsvg_drawing_ctx_pop_render_stack(draw_ctx);
        }

        get_cairo_context(draw_ctx).restore();
    }
}

pub fn get_offset(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_offset(draw_ctx, &mut w, &mut h);
    }

    (w, h)
}

// remove this binding once pangocairo-rs has ContextExt::set_resolution()
fn set_resolution(context: &pango::Context, dpi: f64) {
    unsafe {
        pango_cairo_sys::pango_cairo_context_set_resolution(context.to_glib_none().0, dpi);
    }
}

// remove this binding once pangocairo-rs has ContextExt::set_font_options()
fn set_font_options(context: &pango::Context, options: &cairo::FontOptions) {
    unsafe {
        pango_cairo_sys::pango_cairo_context_set_font_options(
            context.to_glib_none().0,
            options.to_glib_none().0,
        );
    }
}

pub fn get_pango_context(draw_ctx: *const RsvgDrawingCtx) -> pango::Context {
    let font_map = pangocairo::FontMap::get_default().unwrap();
    let context = font_map.create_context().unwrap();
    let cr = get_cairo_context(draw_ctx);
    pangocairo::functions::update_context(&cr, &context);

    set_resolution(&context, get_dpi(draw_ctx).1);

    let testing = unsafe { from_glib(rsvg_drawing_ctx_is_testing(draw_ctx)) };
    if testing {
        let mut options = cairo::FontOptions::new();

        options.set_antialias(cairo::Antialias::Gray);
        options.set_hint_style(cairo::enums::HintStyle::Full);
        options.set_hint_metrics(cairo::enums::HintMetrics::On);

        set_font_options(&context, &options);
    }

    context
}

pub fn insert_bbox(draw_ctx: *const RsvgDrawingCtx, bbox: &BoundingBox) {
    unsafe {
        rsvg_drawing_ctx_insert_bbox(draw_ctx, bbox as *const BoundingBox as *const RsvgBbox);
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

pub fn get_current_state_ptr(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgState {
    unsafe { rsvg_drawing_ctx_get_current_state(draw_ctx) }
}

pub fn get_current_state<'a>(draw_ctx: *const RsvgDrawingCtx) -> Option<&'a State> {
    let state = get_current_state_ptr(draw_ctx);

    if state.is_null() {
        None
    } else {
        Some(state::from_c(state))
    }
}

pub fn get_current_state_mut<'a>(draw_ctx: *const RsvgDrawingCtx) -> Option<&'a mut State> {
    let state = get_current_state_ptr(draw_ctx);

    if state.is_null() {
        None
    } else {
        Some(state::from_c_mut(state))
    }
}

pub fn state_push(draw_ctx: *mut RsvgDrawingCtx) {
    let parent = get_current_state(draw_ctx);

    let mut state = State::new_with_parent(parent);

    if let Some(parent) = parent {
        state.reinherit(parent);
        state.affine = parent.affine;
    }

    unsafe {
        let c_state = Box::into_raw(Box::new(state)) as *mut RsvgState;
        rsvg_drawing_ctx_set_current_state(draw_ctx, c_state);
    }
}

pub fn state_push_not_inherited(draw_ctx: *mut RsvgDrawingCtx) {
    let parent = get_current_state(draw_ctx);

    let state = State::new_with_parent(parent);

    unsafe {
        let c_state = Box::into_raw(Box::new(state)) as *mut RsvgState;
        rsvg_drawing_ctx_set_current_state(draw_ctx, c_state);
    }
}

pub fn state_pop(draw_ctx: *mut RsvgDrawingCtx) {
    let state = get_current_state_mut(draw_ctx).unwrap();

    unsafe {
        let parent = state.parent;
        assert!(!parent.is_null());
        rsvg_drawing_ctx_set_current_state(draw_ctx, parent as *mut _);

        Box::from_raw(state as *mut _);
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

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_transformed_image_bounding_box(
    affine: *const cairo::Matrix,
    w: f64,
    h: f64,
    bbx: *mut libc::c_double,
    bby: *mut libc::c_double,
    bbw: *mut libc::c_double,
    bbh: *mut libc::c_double,
) {
    let affine = unsafe { &*affine };
    let r = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: w,
        height: h,
    }.transform(affine)
        .outer();

    unsafe {
        *bbx = r.x;
        *bby = r.y;
        *bbw = r.width;
        *bbh = r.height;
    }
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_state_push(draw_ctx: *mut RsvgDrawingCtx) {
    state_push(draw_ctx);
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_state_pop(draw_ctx: *mut RsvgDrawingCtx) {
    state_pop(draw_ctx);
}
