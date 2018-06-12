use std::cell::Cell;

use cairo::{self, ImageSurface};

use attributes::Attribute;
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{NodeResult, NodeTrait, RsvgCNodeImpl, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;
use util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::iterators::{ImageSurfaceDataExt, ImageSurfaceDataShared, Pixels};
use super::{make_result, Filter, FilterError, PrimitiveWithInput};

/// The `feOffset` filter primitive.
pub struct Offset {
    base: PrimitiveWithInput,
    dx: Cell<RsvgLength>,
    dy: Cell<RsvgLength>,
}

impl Offset {
    /// Constructs a new `Offset` with empty properties.
    #[inline]
    pub fn new() -> Offset {
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

        let input = make_result(self.base.get_input(ctx))?;
        let bounds = self.base.get_bounds(ctx).add_input(&input).into_irect();

        let dx = self.dx.get().normalize(&values, ctx.drawing_context());
        let dy = self.dy.get().normalize(&values, ctx.drawing_context());
        let paffine = ctx.paffine();
        let ox = (paffine.xx * dx + paffine.xy * dy) as i32;
        let oy = (paffine.yx * dx + paffine.yy * dy) as i32;

        let input_data = ImageSurfaceDataShared::new(input.surface())?;

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
                output_data.set_pixel(output_stride, pixel, output_x, output_y);
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
