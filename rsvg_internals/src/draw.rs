use cairo;

use drawing_ctx::DrawingCtx;
use path_builder::PathBuilder;
use state::{
    ComputedValues,
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

        draw_ctx.stroke_and_fill(&cr, values);
    }
}

