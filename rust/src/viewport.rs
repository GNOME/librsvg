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
                           affine: cairo::Matrix,
                           draw_ctx: *const RsvgDrawingCtx,
                           draw_fn: F)
    where F: FnOnce()
{
    let mut ctx = RsvgDrawingCtxWrapper(draw_ctx);

    in_viewport(&mut ctx,
                vx, vy, vw, vh,
                clip_before_layer_push,
                do_clip,
                vbox,
                preserve_aspect_ratio,
                affine,
                draw_fn);
}

trait ViewportCtx {
    fn push_view_box(&mut self, width: f64, height: f64);
    fn pop_view_box(&mut self, );
    fn push_discrete_layer(&mut self);
    fn pop_discrete_layer(&mut self);
    fn add_clipping_rect(&mut self, x: f64, y: f64, w: f64, h: f64);
    fn set_affine(&mut self, affine: cairo::Matrix);
}

struct RsvgDrawingCtxWrapper(pub *const RsvgDrawingCtx);

impl ViewportCtx for RsvgDrawingCtxWrapper {
    fn push_view_box(&mut self, width: f64, height: f64) {
        drawing_ctx::push_view_box(self.0, width, height);
    }

    fn pop_view_box(&mut self) {
        drawing_ctx::pop_view_box(self.0);
    }

    fn push_discrete_layer(&mut self) {
        drawing_ctx::push_discrete_layer(self.0);
    }

    fn pop_discrete_layer(&mut self) {
        drawing_ctx::pop_discrete_layer(self.0);
    }

    fn add_clipping_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        drawing_ctx::add_clipping_rect(self.0, x, y, w, h);
    }

    fn set_affine(&mut self, affine: cairo::Matrix) {
        drawing_ctx::set_current_state_affine(self.0, affine);
    }
}

fn in_viewport<F>(ctx: &mut ViewportCtx,
                  vx: f64, vy: f64, vw: f64, vh: f64,
                  clip_before_layer_push: bool,
                  do_clip: bool,
                  vbox: Option<ViewBox>,
                  preserve_aspect_ratio: AspectRatio,
                  mut affine: cairo::Matrix,
                  draw_fn: F)
    where F: FnOnce()
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
            ctx.add_clipping_rect(vbox.0.x, vbox.0.y, vbox.0.width, vbox.0.height);
        }
    } else {
        affine.translate(vx, vy);
        vbox_size = (vw, vh);
    }

    ctx.push_view_box(vbox_size.0, vbox_size.1);
    ctx.push_discrete_layer();

    if !clip_before_layer_push && do_clip {
        ctx.add_clipping_rect(vx, vy, vw, vh);
    }

    ctx.set_affine(affine);

    draw_fn();

    ctx.pop_discrete_layer();
    ctx.pop_view_box();
}
