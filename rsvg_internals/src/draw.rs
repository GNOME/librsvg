use cairo;
use cairo_sys;
use glib::translate::*;
use pango::{self, ContextExt, LayoutExt};
use pango_sys;
use pangocairo;

use bbox::BoundingBox;
use drawing_ctx::DrawingCtx;
use float_eq_cairo::ApproxEqCairo;
use length::Dasharray;
use paint_server;
use paint_server::PaintServer;
use path_builder::PathBuilder;
use state::{
    ClipRule,
    CompOp,
    ComputedValues,
    FillRule,
    ShapeRendering,
    StrokeDasharray,
    StrokeLinecap,
    StrokeLinejoin,
    TextRendering,
};

pub fn draw_path_builder(
    draw_ctx: &mut DrawingCtx,
    values: &ComputedValues,
    builder: &PathBuilder,
    clipping: bool,
) {
    let cr = draw_ctx.get_cairo_context();

    draw_ctx.set_affine_on_cr(&cr);
    builder.to_cairo(&cr);

    if clipping {
        cr.set_fill_rule(cairo::FillRule::from(values.clip_rule));
    } else {
        cr.set_fill_rule(cairo::FillRule::from(values.fill_rule));

        stroke_and_fill(&cr, draw_ctx, values);
    }
}

fn stroke_and_fill(cr: &cairo::Context, draw_ctx: &mut DrawingCtx, values: &ComputedValues) {
    cr.set_antialias(cairo::Antialias::from(values.shape_rendering));

    setup_cr_for_stroke(cr, draw_ctx, values);

    // Update the bbox in the rendering context.  Below, we actually set the
    // fill/stroke patterns on the cairo_t.  That process requires the
    // rendering context to have an updated bbox; for example, for the
    // coordinate system in patterns.
    let bbox = compute_stroke_and_fill_box(cr, values);
    draw_ctx.insert_bbox(&bbox);

    let current_color = &values.color.0;

    let fill_opacity = &values.fill_opacity.0;

    if paint_server::set_source_paint_server(
        draw_ctx,
        &values.fill.0,
        fill_opacity,
        &bbox,
        current_color,
    ) {
        if values.stroke.0 == PaintServer::None {
            cr.fill();
        } else {
            cr.fill_preserve();
        }
    }

    let stroke_opacity = values.stroke_opacity.0;

    if paint_server::set_source_paint_server(
        draw_ctx,
        &values.stroke.0,
        &stroke_opacity,
        &bbox,
        &current_color,
    ) {
        cr.stroke();
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

fn setup_cr_for_stroke(cr: &cairo::Context, draw_ctx: &DrawingCtx, values: &ComputedValues) {
    cr.set_line_width(values.stroke_width.0.normalize(values, draw_ctx));
    cr.set_miter_limit(values.stroke_miterlimit.0);
    cr.set_line_cap(cairo::LineCap::from(values.stroke_line_cap));
    cr.set_line_join(cairo::LineJoin::from(values.stroke_line_join));

    if let StrokeDasharray(Dasharray::Array(ref dashes)) = values.stroke_dasharray {
        let normalized_dashes: Vec<f64> = dashes
            .iter()
            .map(|l| l.normalize(values, draw_ctx))
            .collect();

        let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

        if total_length > 0.0 {
            let offset = values.stroke_dashoffset.0.normalize(values, draw_ctx);
            cr.set_dash(&normalized_dashes, offset);
        } else {
            cr.set_dash(&[], 0.0);
        }
    }
}

// remove this binding once cairo-rs has Context::path_extents()
fn path_extents(cr: &cairo::Context) -> (f64, f64, f64, f64) {
    let mut x1: f64 = 0.0;
    let mut y1: f64 = 0.0;
    let mut x2: f64 = 0.0;
    let mut y2: f64 = 0.0;

    unsafe {
        cairo_sys::cairo_path_extents(cr.to_glib_none().0, &mut x1, &mut y1, &mut x2, &mut y2);
    }
    (x1, y1, x2, y2)
}

fn compute_stroke_and_fill_box(cr: &cairo::Context, values: &ComputedValues) -> BoundingBox {
    let affine = cr.get_matrix();

    let mut bbox = BoundingBox::new(&affine);

    // Dropping the precision of cairo's bezier subdivision, yielding 2x
    // _rendering_ time speedups, are these rather expensive operations
    // really needed here? */
    let backup_tolerance = cr.get_tolerance();
    cr.set_tolerance(1.0);

    // Bounding box for fill
    //
    // Unlike the case for stroke, for fills we always compute the bounding box.
    // In GNOME we have SVGs for symbolic icons where each icon has a bounding
    // rectangle with no fill and no stroke, and inside it there are the actual
    // paths for the icon's shape.  We need to be able to compute the bounding
    // rectangle's extents, even when it has no fill nor stroke.

    let fb = BoundingBox::new(&affine).with_ink_extents(cr.fill_extents());
    bbox.insert(&fb);

    // Bounding box for stroke

    if values.stroke.0 != PaintServer::None {
        let sb = BoundingBox::new(&affine).with_ink_extents(cr.stroke_extents());
        bbox.insert(&sb);
    }

    // objectBoundingBox

    let ob = BoundingBox::new(&affine).with_extents(path_extents(cr));
    bbox.insert(&ob);

    // restore tolerance

    cr.set_tolerance(backup_tolerance);

    bbox
}

pub fn draw_pango_layout(
    draw_ctx: &mut DrawingCtx,
    values: &ComputedValues,
    layout: &pango::Layout,
    x: f64,
    y: f64,
    clipping: bool,
) {
    let gravity = layout.get_context().unwrap().get_gravity();

    let (ink, _) = layout.get_extents();

    if ink.width == 0 || ink.height == 0 {
        return;
    }

    let cr = draw_ctx.get_cairo_context();
    cr.save();

    draw_ctx.set_affine_on_cr(&cr);

    let affine = cr.get_matrix();

    let bbox = compute_text_bbox(&ink, x, y, &affine, gravity);

    if !clipping {
        draw_ctx.insert_bbox(&bbox);
    }

    cr.set_antialias(cairo::Antialias::from(values.text_rendering));

    setup_cr_for_stroke(&cr, draw_ctx, values);

    let rotation = unsafe { pango_sys::pango_gravity_to_rotation(gravity.to_glib()) };

    cr.move_to(x, y);
    if !rotation.approx_eq_cairo(&0.0) {
        cr.rotate(-rotation);
    }

    let current_color = &values.color.0;

    let fill_opacity = &values.fill_opacity.0;

    if !clipping {
        if paint_server::set_source_paint_server(
            draw_ctx,
            &values.fill.0,
            fill_opacity,
            &bbox,
            current_color,
        ) {
            pangocairo::functions::update_layout(&cr, layout);
            pangocairo::functions::show_layout(&cr, layout);
        }
    }

    let stroke_opacity = &values.stroke_opacity.0;

    let mut need_layout_path = clipping;

    if !clipping {
        if paint_server::set_source_paint_server(
            draw_ctx,
            &values.stroke.0,
            stroke_opacity,
            &bbox,
            &current_color,
        ) {
            need_layout_path = true;
        }
    }

    if need_layout_path {
        pangocairo::functions::update_layout(&cr, layout);
        pangocairo::functions::layout_path(&cr, layout);

        if !clipping {
            let ib = BoundingBox::new(&affine).with_ink_extents(cr.stroke_extents());
            cr.stroke();
            draw_ctx.insert_bbox(&ib);
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
) -> BoundingBox {
    let pango_scale = f64::from(pango::SCALE);

    let mut bbox = BoundingBox::new(affine);

    let ink_x = f64::from(ink.x);
    let ink_y = f64::from(ink.y);
    let ink_width = f64::from(ink.width);
    let ink_height = f64::from(ink.height);

    if gravity_is_vertical(gravity) {
        bbox.rect = Some(cairo::Rectangle {
            x: x + (ink_x - ink_height) / pango_scale,
            y: y + ink_y / pango_scale,
            width: ink_height / pango_scale,
            height: ink_width / pango_scale,
        });
    } else {
        bbox.rect = Some(cairo::Rectangle {
            x: x + ink_x / pango_scale,
            y: y + ink_y / pango_scale,
            width: ink_width / pango_scale,
            height: ink_height / pango_scale,
        });
    }

    bbox
}
