use cairo;
use glib::translate::*;
use glib_sys;

use drawing_ctx::{self, RsvgDrawingCtx};
use path_builder::RsvgPathBuilder;
use state;
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

fn setup_cr_for_stroke(cairo::Context &cr, draw_ctx: *mut RsvgDrawingCtx, state: *mut RsvgState)
{
    cr.set_line_width();
}

*/
