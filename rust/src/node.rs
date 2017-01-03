extern crate libc;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;

use handle::RsvgHandle;

use property_bag::RsvgPropertyBag;

use state::RsvgState;

pub trait NodeTrait {
    fn set_atts (&self, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag);
    fn draw (&self, draw_ctx: *const RsvgDrawingCtx, dominate: i32);
}
