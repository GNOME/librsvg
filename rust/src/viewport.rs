use cairo;
use cairo::MatrixTrait;

use aspect_ratio::AspectRatio;
use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use util::*;
use viewbox::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    ClipToVbox
}

pub fn draw_in_viewport<F>(vx: f64, vy: f64, vw: f64, vh: f64,
                           clip_mode: ClipMode,
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
                clip_mode,
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
                  clip_mode: ClipMode,
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

    let old_affine = affine;

    if let Some(vbox) = vbox {
        // the preserveAspectRatio attribute is only used if viewBox is specified
        // https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

        let (x, y, w, h) = preserve_aspect_ratio.compute(vbox.0.width, vbox.0.height,
                                                         vx, vy, vw, vh);

        affine.translate(x, y);
        affine.scale(w / vbox.0.width, h / vbox.0.height);
        affine.translate(-vbox.0.x, -vbox.0.y);

        ctx.set_affine(affine);

        ctx.push_view_box(vbox.0.width, vbox.0.height);

        ctx.push_discrete_layer();

        if do_clip && clip_mode == ClipMode::ClipToVbox {
            ctx.add_clipping_rect(vbox.0.x, vbox.0.y, vbox.0.width, vbox.0.height);
        }
    } else {
        affine.translate(vx, vy);
        ctx.set_affine(affine);

        ctx.push_view_box(vw, vh);
        ctx.push_discrete_layer();
    }

    if do_clip && clip_mode == ClipMode::ClipToViewport {
        ctx.set_affine(old_affine);
        ctx.add_clipping_rect(vx, vy, vw, vh);
        ctx.set_affine(affine);
    }

    draw_fn();

    ctx.pop_discrete_layer();
    ctx.pop_view_box();
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsers::Parse;

    #[derive(Default, PartialEq)]
    struct Ctx {
        pub view_box_size: Option<(f64, f64)>,
        pub clipping_rect: Option<(f64, f64, f64, f64)>,
        pub affine:        Option<cairo::Matrix>,

        pub expected_view_box_size: Option<(f64, f64)>,
        pub expected_clipping_rect: Option<(f64, f64, f64, f64)>,
        pub expected_affine:        Option<cairo::Matrix>,
    }

    impl ViewportCtx for Ctx {
        fn push_view_box(&mut self, width: f64, height: f64) {
            self.view_box_size = Some((width, height));
        }

        fn pop_view_box(&mut self) {
        }

        fn push_discrete_layer(&mut self) {
        }

        fn pop_discrete_layer(&mut self) {
        }

        fn add_clipping_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
            self.clipping_rect = Some((x, y, w, h));
        }

        fn set_affine(&mut self, affine: cairo::Matrix) {
            self.affine = Some(affine);
        }
    }

    fn call_in_viewport(vx: f64, vy: f64, vw: f64, vh: f64,
                        clip_mode: ClipMode,
                        do_clip: bool,
                        vbox: Option<ViewBox>,
                        preserve_aspect_ratio: AspectRatio,
                        affine: cairo::Matrix,
                        ctx: &mut Ctx) {
        in_viewport(ctx,
                    vx, vy, vw, vh,
                    clip_mode,
                    do_clip,
                    vbox,
                    preserve_aspect_ratio,
                    affine,
                    || ());

        assert_eq!(ctx.view_box_size, ctx.expected_view_box_size);
        assert_eq!(ctx.clipping_rect, ctx.expected_clipping_rect);
        assert_eq!(ctx.affine, ctx.expected_affine);
    }

    #[test]
    fn clip_after_layer_push() {
        let mut affine = cairo::Matrix::identity();
        affine.scale(0.20, 0.20);

        let mut ctx = Ctx {
            view_box_size: None,
            clipping_rect: None,
            affine:        None,

            expected_view_box_size: Some((50.0, 50.0)),
            expected_clipping_rect: Some((10.0, 10.0, 10.0, 10.0)),
            expected_affine: Some(affine)
        };

        call_in_viewport(10.0, 10.0, 10.0, 10.0,
                         ClipMode::ClipToViewport,
                         true,
                         Some(ViewBox(cairo::Rectangle {
                             x: 50.0,
                             y: 50.0,
                             width: 50.0,
                             height: 50.0,
                         })),
                         AspectRatio::parse("xMidYMid meet", ()).unwrap(),
                         cairo::Matrix::identity(),
                         &mut ctx);
    }

    #[test]
    fn clip_before_layer_push() {
        let mut affine = cairo::Matrix::identity();
        affine.translate(10.0, 10.0);
        affine.scale(0.40, 0.40);

        let mut ctx = Ctx {
            view_box_size: None,
            clipping_rect: None,
            affine:        None,

            expected_view_box_size: Some((50.0, 50.0)),
            expected_clipping_rect: Some((0.0, 0.0, 50.0, 50.0)),
            expected_affine: Some(affine)
        };

        call_in_viewport(10.0, 10.0, 20.0, 20.0,
                         ClipMode::ClipToVbox,
                         true,
                         Some(ViewBox(cairo::Rectangle {
                             x: 0.0,
                             y: 0.0,
                             width: 50.0,
                             height: 50.0,
                         })),
                         AspectRatio::parse("xMidYMid meet", ()).unwrap(),
                         cairo::Matrix::identity(),
                         &mut ctx);
    }
}
