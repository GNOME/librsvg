use cairo;
use cairo::MatrixTrait;

use aspect_ratio::AspectRatio;
use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use util::*;
use viewbox::*;

pub fn draw_in_viewport<F>(vx: f64, vy: f64, vw: f64, vh: f64,
                           clip_before_layer_push: bool,
                           do_clip: bool,
                           vbox: Option<ViewBox>,
                           preserve_aspect_ratio: AspectRatio,
                           mut affine: cairo::Matrix,
                           draw_ctx: *const RsvgDrawingCtx,
                           draw_fn: F)
    where F: FnOnce(cairo::Matrix)
{
    // width or height set to 0 disables rendering of the element
    // https://www.w3.org/TR/SVG/struct.html#SVGElementWidthAttribute
    // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
    // https://www.w3.org/TR/SVG/struct.html#ImageElementWidthAttribute
    // https://www.w3.org/TR/SVG/painting.html#MarkerWidthAttribute

    if double_equals(vw, 0.0) || double_equals(vh, 0.0) {
        return;
    }

    let vbox_size;

    if let Some(vbox) = vbox {
        // the preserveAspectRatio attribute is only used if viewBox is specified
        // https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

        let (x, y, w, h) = preserve_aspect_ratio.compute(vbox.0.width, vbox.0.height,
                                                         vx, vy, vw, vh);

        affine.translate(x, y);
        affine.scale(w / vbox.0.width, h / vbox.0.height);
        affine.translate(-vbox.0.x, -vbox.0.y);

        vbox_size = (vbox.0.width, vbox.0.height);

        if clip_before_layer_push && do_clip {
            drawing_ctx::add_clipping_rect(draw_ctx, vbox.0.x, vbox.0.y, vbox.0.width, vbox.0.height);
        }
    } else {
        affine.translate(vx, vy);
        vbox_size = (vw, vh);
    }

    drawing_ctx::push_view_box(draw_ctx, vbox_size.0, vbox_size.1);
    drawing_ctx::push_discrete_layer(draw_ctx);

    if !clip_before_layer_push && do_clip {
        drawing_ctx::add_clipping_rect(draw_ctx, vx, vy, vw, vh);
    }

    drawing_ctx::set_current_state_affine(draw_ctx, affine);

    draw_fn(affine);

    drawing_ctx::pop_discrete_layer(draw_ctx);
    drawing_ctx::pop_view_box(draw_ctx);
}
