use cairo;
use glib::translate::*;
use pango::{self, ContextExt, LayoutExt};
use pango_sys;
use pangocairo;

use bbox::RsvgBbox;
use drawing_ctx::{self, RsvgDrawingCtx};
use float_eq_cairo::ApproxEqCairo;
use length::Dasharray;
use paint_server;
use path_builder::PathBuilder;
use state::{
    self,
    ClipRule,
    Color,
    CompOp,
    FillRule,
    RsvgState,
    ShapeRendering,
    StrokeDasharray,
    StrokeDashoffset,
    StrokeLinecap,
    StrokeLinejoin,
    StrokeMiterlimit,
    StrokeWidth,
    TextRendering,
};

pub fn draw_path_builder(draw_ctx: *mut RsvgDrawingCtx, builder: &PathBuilder, clipping: bool) {
    if !clipping {
        drawing_ctx::push_discrete_layer(draw_ctx, clipping);
    }

    let state = drawing_ctx::get_current_state(draw_ctx);
    let rstate = state::get_state_rust(state);
    let cr = drawing_ctx::get_cairo_context(draw_ctx);

    drawing_ctx::set_affine_on_cr(draw_ctx, &cr, &rstate.affine);

    builder.to_cairo(&cr);

    if clipping {
        cr.set_fill_rule(cairo::FillRule::from(rstate.clip_rule.unwrap_or_default()));
    } else {
        cr.set_fill_rule(cairo::FillRule::from(rstate.fill_rule.unwrap_or_default()));

        stroke_and_fill(&cr, draw_ctx);

        drawing_ctx::pop_discrete_layer(draw_ctx, clipping);
    }
}

fn stroke_and_fill(cr: &cairo::Context, draw_ctx: *mut RsvgDrawingCtx) {
    let state = drawing_ctx::get_current_state(draw_ctx);
    let rstate = state::get_state_rust(state);

    cr.set_antialias(cairo::Antialias::from(
        rstate.shape_rendering.unwrap_or_default(),
    ));

    setup_cr_for_stroke(cr, draw_ctx, state);

    let bbox = compute_bbox_from_stroke_and_fill(cr, state);

    // Update the bbox in the rendering context.  Below, we actually set the
    // fill/stroke patterns on the cairo_t.  That process requires the
    // rendering context to have an updated bbox; for example, for the
    // coordinate system in patterns.
    drawing_ctx::insert_bbox(draw_ctx, &bbox);

    let fill = state::get_fill(state);
    let stroke = state::get_stroke(state);

    let current_color = rstate
        .color
        .as_ref()
        .map_or_else(|| Color::default().0, |c| c.0);

    if let Some(fill) = fill {
        if paint_server::_set_source_rsvg_paint_server(
            draw_ctx,
            fill,
            state::get_fill_opacity(state),
            &bbox,
            &current_color,
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
            &current_color,
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

impl From<CompOp> for cairo::Operator {
    fn from(op: CompOp) -> cairo::Operator {
        match op {
            CompOp::Clear => cairo::Operator::Clear,
            CompOp::Src => cairo::Operator::Source,
            CompOp::Dst => cairo::Operator::Dest,
            CompOp::SrcOver => cairo::Operator::Over,
            CompOp::DstOver => cairo::Operator::DestOver,
            CompOp::SrcIn => cairo::Operator::In,
            CompOp::DstIn => cairo::Operator::DestIn,
            CompOp::SrcOut => cairo::Operator::Out,
            CompOp::DstOut => cairo::Operator::DestOut,
            CompOp::SrcAtop => cairo::Operator::Atop,
            CompOp::DstAtop => cairo::Operator::DestAtop,
            CompOp::Xor => cairo::Operator::Xor,
            CompOp::Plus => cairo::Operator::Add,
            CompOp::Multiply => cairo::Operator::Multiply,
            CompOp::Screen => cairo::Operator::Screen,
            CompOp::Overlay => cairo::Operator::Overlay,
            CompOp::Darken => cairo::Operator::Darken,
            CompOp::Lighten => cairo::Operator::Lighten,
            CompOp::ColorDodge => cairo::Operator::ColorDodge,
            CompOp::ColorBurn => cairo::Operator::ColorBurn,
            CompOp::HardLight => cairo::Operator::HardLight,
            CompOp::SoftLight => cairo::Operator::SoftLight,
            CompOp::Difference => cairo::Operator::Difference,
            CompOp::Exclusion => cairo::Operator::Exclusion,
        }
    }
}

impl From<ClipRule> for cairo::FillRule {
    fn from(c: ClipRule) -> cairo::FillRule {
        match c {
            ClipRule::NonZero => cairo::FillRule::Winding,
            ClipRule::EvenOdd => cairo::FillRule::EvenOdd,
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

impl From<ShapeRendering> for cairo::Antialias {
    fn from(sr: ShapeRendering) -> cairo::Antialias {
        match sr {
            ShapeRendering::Auto | ShapeRendering::GeometricPrecision => cairo::Antialias::Default,
            ShapeRendering::OptimizeSpeed | ShapeRendering::CrispEdges => cairo::Antialias::None,
        }
    }
}

impl From<TextRendering> for cairo::Antialias {
    fn from(tr: TextRendering) -> cairo::Antialias {
        match tr {
            TextRendering::Auto
            | TextRendering::OptimizeLegibility
            | TextRendering::GeometricPrecision => cairo::Antialias::Default,
            TextRendering::OptimizeSpeed => cairo::Antialias::None,
        }
    }
}

fn setup_cr_for_stroke(
    cr: &cairo::Context,
    draw_ctx: *const RsvgDrawingCtx,
    state: *mut RsvgState,
) {
    let rstate = state::get_state_rust(state);

    cr.set_line_width(
        rstate
            .stroke_width
            .as_ref()
            .map_or_else(|| StrokeWidth::default().0, |w| w.0)
            .normalize(draw_ctx),
    );
    cr.set_miter_limit(
        rstate
            .stroke_miterlimit
            .as_ref()
            .map_or_else(|| StrokeMiterlimit::default().0, |l| l.0),
    );
    cr.set_line_cap(cairo::LineCap::from(
        rstate.stroke_line_cap.unwrap_or_default(),
    ));
    cr.set_line_join(cairo::LineJoin::from(
        rstate.stroke_line_join.unwrap_or_default(),
    ));

    match rstate.stroke_dasharray {
        Some(StrokeDasharray(Dasharray::Array(ref dashes))) => {
            let normalized_dashes: Vec<f64> =
                dashes.iter().map(|l| l.normalize(draw_ctx)).collect();

            let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

            if total_length > 0.0 {
                let offset = rstate
                    .stroke_dashoffset
                    .as_ref()
                    .map_or_else(|| StrokeDashoffset::default().0, |o| o.0)
                    .normalize(draw_ctx);
                cr.set_dash(&normalized_dashes, offset);
            } else {
                cr.set_dash(&[], 0.0);
            }
        }

        _ => {
            cr.set_dash(&[], 0.0);
        }
    }
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
    let rstate = state::get_state_rust(state);

    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    let gravity = layout.get_context().unwrap().get_gravity();

    let (ink, _) = layout.get_extents();

    if ink.width == 0 || ink.height == 0 {
        return;
    }

    let bbox = compute_text_bbox(&ink, x, y, &rstate.affine, gravity);

    let fill = state::get_fill(state);
    let stroke = state::get_stroke(state);

    if !clipping && (fill.is_some() || stroke.is_some()) {
        drawing_ctx::insert_bbox(draw_ctx, &bbox);
    }

    cr.set_antialias(cairo::Antialias::from(
        rstate.text_rendering.unwrap_or_default(),
    ));

    setup_cr_for_stroke(&cr, draw_ctx, state);

    drawing_ctx::set_affine_on_cr(draw_ctx, &cr, &rstate.affine);

    let rotation = unsafe { pango_sys::pango_gravity_to_rotation(gravity.to_glib()) };

    cr.save();
    cr.move_to(x, y);
    if !rotation.approx_eq_cairo(&0.0) {
        cr.rotate(-rotation);
    }

    let current_color = rstate
        .color
        .as_ref()
        .map_or_else(|| Color::default().0, |c| c.0);

    if !clipping {
        if let Some(fill) = fill {
            if paint_server::_set_source_rsvg_paint_server(
                draw_ctx,
                fill,
                state::get_fill_opacity(state),
                &bbox,
                &current_color,
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
                &current_color,
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

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() ?
fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    match gravity {
        pango::Gravity::East | pango::Gravity::West => true,
        _ => false,
    }
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

    if gravity_is_vertical(gravity) {
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
    let rstate = state::get_state_rust(state);

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

    cr.set_operator(cairo::Operator::from(rstate.comp_op.unwrap_or_default()));

    cr.set_source_surface(&surface, x, y);
    cr.paint();

    drawing_ctx::insert_bbox(draw_ctx, &bbox);
}

pub fn add_clipping_rect(draw_ctx: *mut RsvgDrawingCtx, x: f64, y: f64, w: f64, h: f64) {
    let state = drawing_ctx::get_current_state(draw_ctx);
    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    let affine = state::get_state_rust(state).affine;

    drawing_ctx::set_affine_on_cr(draw_ctx, &cr, &affine);

    cr.rectangle(x, y, w, h);
    cr.clip();
}

#[no_mangle]
pub extern "C" fn rsvg_cairo_add_clipping_rect(
    draw_ctx: *mut RsvgDrawingCtx,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    add_clipping_rect(draw_ctx, x, y, w, h);
}
