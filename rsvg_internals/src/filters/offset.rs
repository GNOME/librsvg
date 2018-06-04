use std::cell::Cell;

use cairo::{self, ImageSurface};
use libc::{self, c_char};

use attributes::Attribute;
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{boxed_node_new, NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;
use util::{clamp, utf8_cstr_opt};

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::iterators::{ImageSurfaceDataShared, Pixels};
use super::{get_surface, Filter, FilterError, PrimitiveWithInput};

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
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
    }
}

impl Filter for Offset {
    fn render(&self, node: &RsvgNode, ctx: &FilterContext) -> Result<FilterResult, FilterError> {
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        let bounds = self.base.get_bounds(ctx);

        let dx = self.dx.get().normalize(&values, ctx.drawing_context());
        let dy = self.dy.get().normalize(&values, ctx.drawing_context());
        let paffine = ctx.paffine();
        let ox = (paffine.xx * dx + paffine.xy * dy) as i32;
        let oy = (paffine.yx * dx + paffine.yy * dy) as i32;

        let input_surface = get_surface(self.base.get_input(ctx))?;
        let input_data = ImageSurfaceDataShared::new(&input_surface)?;

        // input_bounds contains all pixels within bounds,
        // for which (x + ox) and (y + oy) also lie within bounds.
        let input_bounds = IRect {
            x0: clamp(bounds.x0 - ox, bounds.x0, bounds.x1),
            y0: clamp(bounds.y0 - oy, bounds.y0, bounds.y1),
            x1: clamp(bounds.x1 - ox, bounds.x0, bounds.x1),
            y1: clamp(bounds.y1 - oy, bounds.y0, bounds.y1),
        };

        let mut output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            input_data.width as i32,
            input_data.height as i32,
        ).map_err(FilterError::OutputSurfaceCreation)?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, pixel) in Pixels::new(input_data, input_bounds) {
                let output_x = (x as i32 + ox) as usize;
                let output_y = (y as i32 + oy) as usize;

                let output_base = output_y * output_stride + output_x * 4;
                output_data[output_base + 0] = pixel.r;
                output_data[output_base + 1] = pixel.g;
                output_data[output_base + 2] = pixel.b;
                output_data[output_base + 3] = pixel.a;
            }
        }

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: output_surface,
                bounds,
            },
        })
    }
}

/// Returns a new `feOffset` node.
#[no_mangle]
pub unsafe extern "C" fn rsvg_new_filter_primitive_offset(
    _element_name: *const c_char,
    parent: *mut RsvgNode,
    id: *const libc::c_char,
) -> *mut RsvgNode {
    let filter = Offset::new();
    boxed_node_new(
        NodeType::FilterPrimitiveOffset,
        parent,
        utf8_cstr_opt(id),
        Box::new(filter),
    )
}
