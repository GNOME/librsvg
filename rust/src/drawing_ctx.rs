pub enum RsvgDrawingCtx {}

extern "C" {
    fn rsvg_drawing_ctx_get_view_box_size (draw_ctx: *const RsvgDrawingCtx,
                                           out_x: *mut f64,
                                           out_y: *mut f64);
}

pub fn get_view_box_size (draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe { rsvg_drawing_ctx_get_view_box_size (draw_ctx, &mut w, &mut h); }

    (w, h)
}
