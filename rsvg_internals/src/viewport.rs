use cairo::Rectangle;

use aspect_ratio::AspectRatio;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use node::RsvgNode;
use properties::ComputedValues;
use viewbox::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    ClipToVbox,
}

pub fn draw_in_viewport(
    viewport: &Rectangle,
    clip_mode: Option<ClipMode>,
    vbox: Option<ViewBox>,
    preserve_aspect_ratio: AspectRatio,
    node: &RsvgNode,
    values: &ComputedValues,
    draw_ctx: &mut DrawingCtx,
    clipping: bool,
    draw_fn: &mut FnMut(&mut DrawingCtx) -> Result<(), RenderingError>,
) -> Result<(), RenderingError> {
    draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
        let _params = dc.push_new_viewport(vbox, viewport, preserve_aspect_ratio, clip_mode);

        let res = draw_fn(dc);

        res
    })
}
