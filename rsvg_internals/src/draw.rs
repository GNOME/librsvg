use cairo;
use cairo_sys;
use glib::translate::*;
use glib_sys;

use bbox::RsvgBbox;
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

    let bbox = compute_bbox_from_stroke_and_fill(cr, state);

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

fn compute_bbox_from_stroke_and_fill(cr: &cairo::Context, state: *mut RsvgState) -> RsvgBbox {
    let state_affine = &state::get_affine(state);

    let mut bbox = RsvgBbox::new(state_affine);

    // Dropping the precision of cairo's bezier subdivision, yielding 2x
    // _rendering_ time speedups, are these rather expensive operations
    // really needed here? */
    let backup_tolerance = cr.get_tolerance();
    cr.set_tolerance(1.0);

    // FIXME: See https://www.w3.org/TR/SVG/coords.html#ObjectBoundingBox for
    // discussion on how to compute bounding boxes to be used for viewports and
    // clipping.  It looks like we should be using cairo_path_extents() for
    // that, not cairo_fill_extents().
    //
    // We may need to maintain *two* sets of bounding boxes - one for
    // viewports/clipping, and one for user applications like a
    // rsvg_compute_ink_rect() function in the future.
    //
    // See https://gitlab.gnome.org/GNOME/librsvg/issues/128 for discussion of a
    // public API to get the ink rectangle.

    // Bounding box for fill
    //
    // Unlike the case for stroke, for fills we always compute the bounding box.
    // In GNOME we have SVGs for symbolic icons where each icon has a bounding
    // rectangle with no fill and no stroke, and inside it there are the actual
    // paths for the icon's shape.  We need to be able to compute the bounding
    // rectangle's extents, even when it has no fill nor stroke.

    {
        let mut fb = RsvgBbox::new(state_affine);

        let (x, y, w, h) = cr.fill_extents();

        fb.set_rect(&cairo::Rectangle {
            x,
            y,
            width: w - x,
            height: h - y,
        });

        bbox.insert(&fb);
    }

    // Bounding box for stroke

    if state::get_stroke(state).is_some() {
        let mut sb = RsvgBbox::new(state_affine);

        let (x, y, w, h) = cr.stroke_extents();

        sb.set_rect(&cairo::Rectangle {
            x,
            y,
            width: w - x,
            height: h - y
        });

        bbox.insert(&sb);
    }

    cr.set_tolerance(backup_tolerance);

    bbox
}

#[no_mangle]
pub extern "C" fn rsvg_setup_cr_for_stroke(cr: *mut cairo_sys::cairo_t,
                                           draw_ctx: *mut RsvgDrawingCtx,
                                           state: *mut RsvgState) {
    let cr = unsafe { cairo::Context::from_glib_none(cr) };

    setup_cr_for_stroke(&cr, draw_ctx, state);
}

#[no_mangle]
pub extern "C" fn rsvg_compute_bbox_from_stroke_and_fill(cr: *mut cairo_sys::cairo_t,
                                                         state: *mut RsvgState) -> RsvgBbox {
    let cr = unsafe { cairo::Context::from_glib_none(cr) };

    compute_bbox_from_stroke_and_fill(&cr, state)
}
