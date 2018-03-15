use cairo;
use cairo_sys;
use glib::translate::*;
use glib_sys;

use drawing_ctx::{self, RsvgDrawingCtx};
use length::StrokeDasharray;
use path_builder::RsvgPathBuilder;
use state::{self, RsvgState};

/*
#[no_mangle]
pub extern "C" fn rsvg_draw_path_builder(draw_ctx: *mut RsvgDrawingCtx,
                                         raw_builder: *const RsvgPathBuilder,
                                         clipping: glib_sys::gboolean)
{
    assert!(!draw_ctx.is_null());
    assert!(!raw_builder.is_null());

    let builder = unsafe { &*raw_builder };
    let clipping: bool = from_glib(clipping);

    if !clipping {
        drawing_ctx::push_discrete_layer(draw_ctx);
    }

    let state = drawing_ctx::get_current_state(draw_ctx);
    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    let affine = drawing_ctx::get_current_state_affine(draw_ctx);

    drawing_ctx::set_affine_on_cr(draw_ctx, &cr, &affine);

    builder.to_cairo(&cr);

    if clipping {
        cr.set_fill_rule(state::get_clip_rule(state));
    } else {
        cr.set_fill_rule(state::get_fill_rule(state));

        stroke_and_fill(&cr, draw_ctx);

        drawing_ctx::pop_discrete_layer(draw_ctx);
    }
}

fn stroke_and_fill(cr: &cairo::Context, draw_ctx: *mut RsvgDrawingCtx) {
    let state = drawing_ctx::get_current_state(draw_ctx);

    cr.set_antialias(state::get_shape_rendering_type(state));

    setup_cr_for_stroke(cr, draw_ctx, state);

    FIXME
}
*/

fn setup_cr_for_stroke(cr: &cairo::Context, draw_ctx: *mut RsvgDrawingCtx, state: *mut RsvgState)
{
    cr.set_line_width(state::get_stroke_width(state).normalize(draw_ctx));
    cr.set_miter_limit(state::get_miter_limit(state));
    cr.set_line_cap(state::get_line_cap(state));
    cr.set_line_join(state::get_line_join(state));

    let dash = state::get_stroke_dasharray(state);

    dash.unwrap_or(&StrokeDasharray::None)
        .set_on_cairo(draw_ctx, cr, &state::get_dash_offset(state));
}

#[no_mangle]
pub extern "C" fn rsvg_setup_cr_for_stroke(cr: *mut cairo_sys::cairo_t,
                                           draw_ctx: *mut RsvgDrawingCtx,
                                           state: *mut RsvgState) {
    let cr = unsafe { cairo::Context::from_glib_none(cr) };

    setup_cr_for_stroke(&cr, draw_ctx, state);
}
