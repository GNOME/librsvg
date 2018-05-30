use std::cell::Cell;
use std::slice;

use cairo::prelude::SurfaceExt;
use cairo::{self, ImageSurface};
use cairo_sys;
use libc::c_char;

use attributes::Attribute;
use drawing_ctx::RsvgDrawingCtx;
use filter_context::{FilterContext, FilterResult};
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{
    boxed_node_new,
    CascadedValues,
    NodeResult,
    NodeTrait,
    NodeType,
    RsvgCNodeImpl,
    RsvgNode,
};
use parsers::{parse, Parse};
use property_bag::PropertyBag;

use super::{Filter, PrimitiveWithInput};

/// The `feOffset` filter primitive.
struct Offset {
    base: PrimitiveWithInput,
    dx: Cell<RsvgLength>,
    dy: Cell<RsvgLength>,
}

impl Offset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    fn new() -> Offset {
        Offset {
            base: PrimitiveWithInput::new::<Self>(),
            dx: Cell::new(RsvgLength::parse("0", LengthDir::Horizontal).unwrap()),
            dy: Cell::new(RsvgLength::parse("0", LengthDir::Vertical).unwrap()),
        }
    }
}

impl NodeTrait for Offset {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Dx => self
                    .dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    #[inline]
    fn draw(&self, _: &RsvgNode, _: &CascadedValues, _: *mut RsvgDrawingCtx, _: bool, _: bool) {
        // Nothing; filters are drawn in rsvg-cairo-draw.c.
    }

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
    }
}

impl Filter for Offset {
    fn render(&self, node: &RsvgNode, ctx: &mut FilterContext) {
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        let bounds = self.base.get_bounds(ctx);

        let dx = self.dx.get().normalize(&values, ctx.drawing_context());
        let dy = self.dy.get().normalize(&values, ctx.drawing_context());
        let paffine = ctx.paffine();
        let ox = (paffine.xx * dx + paffine.xy * dy) as i32;
        let oy = (paffine.yx * dx + paffine.yy * dy) as i32;

        let input_surface = match self.base.get_input(ctx) {
            Some(FilterResult { surface, .. }) => surface,
            None => return,
        };

        let width = input_surface.get_width();
        let height = input_surface.get_height();
        let input_stride = input_surface.get_stride();

        // TODO: this currently gives "non-exclusive access" (can we make read-only borrows?)
        // let input_data = input_surface.get_data().unwrap();
        input_surface.flush();
        if input_surface.status() != cairo::Status::Success {
            return;
        }
        let input_data_ptr =
            unsafe { cairo_sys::cairo_image_surface_get_data(input_surface.to_raw_none()) };
        if input_data_ptr.is_null() {
            return;
        }
        let input_data_len = input_stride as usize * height as usize;
        let input_data = unsafe { slice::from_raw_parts(input_data_ptr, input_data_len) };

        let mut output_surface = match ImageSurface::create(cairo::Format::ARgb32, width, height) {
            Ok(surface) => surface,
            Err(_) => return,
        };

        let output_stride = output_surface.get_stride();
        {
            let mut output_data = output_surface.get_data().unwrap();

            for y in bounds.y0..bounds.y1 {
                for x in bounds.x0..bounds.x1 {
                    if x - ox < bounds.x0
                        || x - ox >= bounds.x1
                        || y - oy < bounds.y0
                        || y - oy >= bounds.y1
                    {
                        continue;
                    }

                    for ch in 0..4 {
                        let input_index = ((y - oy) * input_stride + (x - ox) * 4 + ch) as usize;
                        let output_index = (y * output_stride + x * 4 + ch) as usize;

                        output_data[output_index] = input_data[input_index];
                    }
                }
            }
        }

        ctx.store_result(
            self.base.result.borrow().clone(),
            FilterResult {
                surface: output_surface,
                bounds,
            },
        );
    }
}

/// Returns a new `feOffset` node.
#[no_mangle]
pub unsafe extern "C" fn rsvg_new_filter_primitive_offset(
    _element_name: *const c_char,
    parent: *mut RsvgNode,
) -> *mut RsvgNode {
    let filter = Offset::new();
    boxed_node_new(NodeType::FilterPrimitiveOffset, parent, Box::new(filter))
}
