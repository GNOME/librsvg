use cairo;
use cairo::MatrixTrait;

use aspect_ratio::AspectRatio;
use draw::add_clipping_rect;
use drawing_ctx::{self, RsvgDrawingCtx};
use float_eq_cairo::ApproxEqCairo;
use node::RsvgNode;
use state::ComputedValues;
use viewbox::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    ClipToVbox,
}

pub fn draw_in_viewport<F>(
    vx: f64,
    vy: f64,
    vw: f64,
    vh: f64,
    clip_mode: ClipMode,
    do_clip: bool,
    vbox: Option<ViewBox>,
    preserve_aspect_ratio: AspectRatio,
    node: &RsvgNode,
    values: &ComputedValues,
    mut affine: cairo::Matrix,
    draw_ctx: *mut RsvgDrawingCtx,
    clipping: bool,
    draw_fn: F,
) where
    F: FnOnce(),
{
    // width or height set to 0 disables rendering of the element
    // https://www.w3.org/TR/SVG/struct.html#SVGElementWidthAttribute
    // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
    // https://www.w3.org/TR/SVG/struct.html#ImageElementWidthAttribute
    // https://www.w3.org/TR/SVG/painting.html#MarkerWidthAttribute

    if vw.approx_eq_cairo(&0.0) || vh.approx_eq_cairo(&0.0) {
        return;
    }

    drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);

    if do_clip && clip_mode == ClipMode::ClipToViewport {
        drawing_ctx::get_cairo_context(draw_ctx).set_matrix(affine);
        add_clipping_rect(draw_ctx, vx, vy, vw, vh);
    }

    if let Some(vbox) = vbox {
        // the preserveAspectRatio attribute is only used if viewBox is specified
        // https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

        if vbox.0.width.approx_eq_cairo(&0.0) || vbox.0.height.approx_eq_cairo(&0.0) {
            // Width or height of 0 for the viewBox disables rendering of the element
            // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
            return;
        }

        drawing_ctx::push_view_box(draw_ctx, vbox.0.width, vbox.0.height);

        let (x, y, w, h) =
            preserve_aspect_ratio.compute(vbox.0.width, vbox.0.height, vx, vy, vw, vh);

        affine.translate(x, y);
        affine.scale(w / vbox.0.width, h / vbox.0.height);
        affine.translate(-vbox.0.x, -vbox.0.y);

        drawing_ctx::get_cairo_context(draw_ctx).set_matrix(affine);

        if do_clip && clip_mode == ClipMode::ClipToVbox {
            add_clipping_rect(draw_ctx, vbox.0.x, vbox.0.y, vbox.0.width, vbox.0.height);
        }
    } else {
        drawing_ctx::push_view_box(draw_ctx, vw, vh);

        affine.translate(vx, vy);
        drawing_ctx::get_cairo_context(draw_ctx).set_matrix(affine);
    }

    draw_fn();

    drawing_ctx::pop_view_box(draw_ctx);
    drawing_ctx::pop_discrete_layer(draw_ctx, node, values, clipping);
}
