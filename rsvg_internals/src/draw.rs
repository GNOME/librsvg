use cairo;
use glib::translate::*;
use pango::{self, ContextExt, LayoutExt};
use pango_sys;
use pangocairo;

use bbox::RsvgBbox;
use drawing_ctx::{self, RsvgDrawingCtx};
use float_eq_cairo::ApproxEqCairo;
use length::StrokeDasharray;
use paint_server;
use path_builder::RsvgPathBuilder;
use state::{self, FillRule, RsvgState, StrokeLinecap, StrokeLinejoin};
use text;

pub fn draw_path_builder(draw_ctx: *mut RsvgDrawingCtx, builder: &RsvgPathBuilder, clipping: bool) {
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
        cr.set_fill_rule(cairo::FillRule::from(
            state::get_state_rust(state).fill_rule.unwrap_or_default(),
        ));

        stroke_and_fill(&cr, draw_ctx);

        drawing_ctx::pop_discrete_layer(draw_ctx);
    }
}

fn stroke_and_fill(cr: &cairo::Context, draw_ctx: *mut RsvgDrawingCtx) {
    let state = drawing_ctx::get_current_state(draw_ctx);

    cr.set_antialias(state::get_shape_rendering_type(state));

    setup_cr_for_stroke(cr, draw_ctx, state);

    let bbox = compute_bbox_from_stroke_and_fill(cr, state);

    // Update the bbox in the rendering context.  Below, we actually set the fill/stroke
    // patterns on the cairo_t.  That process requires the rendering context to have
    // an updated bbox; for example, for the coordinate system in patterns.
    drawing_ctx::insert_bbox(draw_ctx, &bbox);

    let fill = state::get_fill(state);
    let stroke = state::get_stroke(state);

    if let Some(fill) = fill {
        if paint_server::_set_source_rsvg_paint_server(
            draw_ctx,
            fill,
            state::get_fill_opacity(state),
            &bbox,
            state::get_current_color(state),
        ) {
            if stroke.is_some() {
                cr.fill_preserve();
            } else {
                cr.fill();
            }
        }
    }

    if let Some(stroke) = stroke {
        if paint_server::_set_source_rsvg_paint_server(
            draw_ctx,
            stroke,
            state::get_stroke_opacity(state),
            &bbox,
            state::get_current_color(state),
        ) {
            cr.stroke();
        }
    }

    // clear the path in case stroke == fill == None; otherwise
    // we leave it around from computing the bounding box
    cr.new_path();
}

impl From<StrokeLinejoin> for cairo::LineJoin {
    fn from(j: StrokeLinejoin) -> cairo::LineJoin {
        match j {
            StrokeLinejoin::Miter => cairo::LineJoin::Miter,
            StrokeLinejoin::Round => cairo::LineJoin::Round,
            StrokeLinejoin::Bevel => cairo::LineJoin::Bevel,
        }
    }
}

impl From<StrokeLinecap> for cairo::LineCap {
    fn from(j: StrokeLinecap) -> cairo::LineCap {
        match j {
            StrokeLinecap::Butt => cairo::LineCap::Butt,
            StrokeLinecap::Round => cairo::LineCap::Round,
            StrokeLinecap::Square => cairo::LineCap::Square,
        }
    }
}

impl From<FillRule> for cairo::FillRule {
    fn from(f: FillRule) -> cairo::FillRule {
        match f {
            FillRule::NonZero => cairo::FillRule::Winding,
            FillRule::EvenOdd => cairo::FillRule::EvenOdd,
        }
    }
}

fn setup_cr_for_stroke(
    cr: &cairo::Context,
    draw_ctx: *const RsvgDrawingCtx,
    state: *mut RsvgState,
) {
    cr.set_line_width(state::get_stroke_width(state).normalize(draw_ctx));
    cr.set_miter_limit(state::get_miter_limit(state));
    cr.set_line_cap(cairo::LineCap::from(
        state::get_state_rust(state).cap.unwrap_or_default(),
    ));
    cr.set_line_join(cairo::LineJoin::from(
        state::get_state_rust(state).join.unwrap_or_default(),
    ));

    let dash = state::get_stroke_dasharray(state);

    dash.unwrap_or(&StrokeDasharray::None).set_on_cairo(
        draw_ctx,
        cr,
        &state::get_dash_offset(state),
    );
}

fn compute_bbox_from_stroke_and_fill(cr: &cairo::Context, state: *mut RsvgState) -> RsvgBbox {
    let rstate = state::get_state_rust(state);

    let mut bbox = RsvgBbox::new(&rstate.affine);

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
        let mut fb = RsvgBbox::new(&rstate.affine);

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
        let mut sb = RsvgBbox::new(&rstate.affine);

        let (x, y, w, h) = cr.stroke_extents();

        sb.set_rect(&cairo::Rectangle {
            x,
            y,
            width: w - x,
            height: h - y,
        });

        bbox.insert(&sb);
    }

    cr.set_tolerance(backup_tolerance);

    bbox
}

pub fn draw_pango_layout(
    draw_ctx: *mut RsvgDrawingCtx,
    layout: &pango::Layout,
    x: f64,
    y: f64,
    clipping: bool,
) {
    let state = drawing_ctx::get_current_state(draw_ctx);
    let rust_state = state::get_state_rust(state);
    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    let gravity = layout.get_context().unwrap().get_gravity();

    let (ink, _) = layout.get_extents();

    if ink.width == 0 || ink.height == 0 {
        return;
    }

    let bbox = compute_text_bbox(&ink, x, y, &rust_state.affine, gravity);

    let fill = state::get_fill(state);
    let stroke = state::get_stroke(state);

    if !clipping && (fill.is_some() || stroke.is_some()) {
        drawing_ctx::insert_bbox(draw_ctx, &bbox);
    }

    cr.set_antialias(state::get_text_rendering_type(state));

    setup_cr_for_stroke(&cr, draw_ctx, state);

    drawing_ctx::set_affine_on_cr(draw_ctx, &cr, &rust_state.affine);

    let rotation = unsafe { pango_sys::pango_gravity_to_rotation(gravity.to_glib()) };

    cr.save();
    cr.move_to(x, y);
    if !rotation.approx_eq_cairo(&0.0) {
        cr.rotate(-rotation);
    }

    if !clipping {
        if let Some(fill) = fill {
            if paint_server::_set_source_rsvg_paint_server(
                draw_ctx,
                fill,
                state::get_fill_opacity(state),
                &bbox,
                state::get_current_color(state),
            ) {
                pangocairo::functions::update_layout(&cr, layout);
                pangocairo::functions::show_layout(&cr, layout);
            }
        }
    }

    let need_layout_path;

    if clipping {
        need_layout_path = true;
    } else {
        need_layout_path = stroke.is_some()
            && paint_server::_set_source_rsvg_paint_server(
                draw_ctx,
                stroke.unwrap(),
                state::get_stroke_opacity(state),
                &bbox,
                state::get_current_color(state),
            );
    }

    if need_layout_path {
        pangocairo::functions::update_layout(&cr, layout);
        pangocairo::functions::layout_path(&cr, layout);

        if !clipping {
            cr.stroke();
        }
    }

    cr.restore();
}

fn compute_text_bbox(
    ink: &pango::Rectangle,
    x: f64,
    y: f64,
    affine: &cairo::Matrix,
    gravity: pango::Gravity,
) -> RsvgBbox {
    let pango_scale = f64::from(pango::SCALE);

    let mut bbox = RsvgBbox::new(affine);

    let ink_x = f64::from(ink.x);
    let ink_y = f64::from(ink.y);
    let ink_width = f64::from(ink.width);
    let ink_height = f64::from(ink.height);

    if text::gravity_is_vertical(gravity) {
        bbox.set_rect(&cairo::Rectangle {
            x: x + (ink_x - ink_height) / pango_scale,
            y: y + ink_y / pango_scale,
            width: ink_height / pango_scale,
            height: ink_width / pango_scale,
        });
    } else {
        bbox.set_rect(&cairo::Rectangle {
            x: x + ink_x / pango_scale,
            y: y + ink_y / pango_scale,
            width: ink_width / pango_scale,
            height: ink_height / pango_scale,
        });
    }

    bbox
}

pub fn draw_surface(
    draw_ctx: *mut RsvgDrawingCtx,
    surface: &cairo::ImageSurface,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    clipping: bool,
) {
    if clipping {
        return;
    }

    let state = drawing_ctx::get_current_state(draw_ctx);
    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    let affine = state::get_state_rust(state).affine;

    let width = surface.get_width();
    let height = surface.get_height();

    if width == 0 || height == 0 {
        return;
    }

    let width = f64::from(width);
    let height = f64::from(height);

    let mut bbox = RsvgBbox::new(&affine);
    bbox.set_rect(&cairo::Rectangle {
        x,
        y,
        width,
        height,
    });

    drawing_ctx::set_affine_on_cr(draw_ctx, &cr, &affine);
    cr.scale(w / width, h / height);
    let x = x * width / w;
    let y = y * height / h;

    cr.set_operator(state::get_comp_op(state));

    cr.set_source_surface(&surface, x, y);
    cr.paint();

    drawing_ctx::insert_bbox(draw_ctx, &bbox);
}
