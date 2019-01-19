use cairo;
use cairo::MatrixTrait;

use aspect_ratio::AspectRatio;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use float_eq_cairo::ApproxEqCairo;
use node::RsvgNode;
use properties::ComputedValues;
use viewbox::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    ClipToVbox,
}

pub fn draw_in_viewport(
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
    draw_ctx: &mut DrawingCtx,
    clipping: bool,
    draw_fn: &mut FnMut(&mut DrawingCtx) -> Result<(), RenderingError>,
) -> Result<(), RenderingError> {
    // width or height set to 0 disables rendering of the element
    // https://www.w3.org/TR/SVG/struct.html#SVGElementWidthAttribute
    // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
    // https://www.w3.org/TR/SVG/struct.html#ImageElementWidthAttribute
    // https://www.w3.org/TR/SVG/painting.html#MarkerWidthAttribute

    if vw.approx_eq_cairo(&0.0) || vh.approx_eq_cairo(&0.0) {
        return Ok(());
    }

    draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
        if do_clip && clip_mode == ClipMode::ClipToViewport {
            dc.get_cairo_context().set_matrix(affine);
            dc.clip(vx, vy, vw, vh);
        }

        let _params = if let Some(vbox) = vbox {
            // the preserveAspectRatio attribute is only used if viewBox is specified
            // https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

            if vbox.width.approx_eq_cairo(&0.0) || vbox.height.approx_eq_cairo(&0.0) {
                // Width or height of 0 for the viewBox disables rendering of the element
                // https://www.w3.org/TR/SVG/coords.html#ViewBoxAttribute
                return Ok(());
            }

            let params = dc.push_view_box(vbox.width, vbox.height);

            let (x, y, w, h) =
                preserve_aspect_ratio.compute(vbox.width, vbox.height, vx, vy, vw, vh);

            affine.translate(x, y);
            affine.scale(w / vbox.width, h / vbox.height);
            affine.translate(-vbox.x, -vbox.y);

            dc.get_cairo_context().set_matrix(affine);

            if do_clip && clip_mode == ClipMode::ClipToVbox {
                dc.clip(vbox.x, vbox.y, vbox.width, vbox.height);
            }

            params
        } else {
            let params = dc.push_view_box(vw, vh);
            affine.translate(vx, vy);
            dc.get_cairo_context().set_matrix(affine);
            params
        };

        let res = draw_fn(dc);

        res
    })
}
