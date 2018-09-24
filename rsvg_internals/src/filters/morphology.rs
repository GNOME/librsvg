use std::cell::Cell;
use std::cmp::{max, min};

use cairo::{self, ImageSurface, MatrixTrait};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{self, ParseError};
use property_bag::PropertyBag;
use surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::SharedImageSurface,
    EdgeMode,
    ImageSurfaceDataExt,
    Pixel,
};

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::{Filter, FilterError, PrimitiveWithInput};

/// Enumeration of the possible morphology operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum Operator {
    Erode,
    Dilate,
}

/// The `feMorphology` filter primitive.
pub struct Morphology {
    base: PrimitiveWithInput,
    operator: Cell<Operator>,
    radius: Cell<(f64, f64)>,
}

impl Morphology {
    /// Constructs a new `Morphology` with empty properties.
    #[inline]
    pub fn new() -> Morphology {
        Morphology {
            base: PrimitiveWithInput::new::<Self>(),
            operator: Cell::new(Operator::Erode),
            radius: Cell::new((0.0, 0.0)),
        }
    }
}

impl NodeTrait for Morphology {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Operator => self.operator.set(Operator::parse(attr, value)?),
                Attribute::Radius => self.radius.set(
                    parsers::number_optional_number(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|(x, y)| {
                            if x >= 0.0 && y >= 0.0 {
                                Ok((x, y))
                            } else {
                                Err(NodeError::value_error(attr, "radius cannot be negative"))
                            }
                        })?,
                ),
                _ => (),
            }
        }

        Ok(())
    }
}

impl Filter for Morphology {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);

        let (rx, ry) = self.radius.get();
        let (rx, ry) = ctx.paffine().transform_distance(rx, ry);

        let operator = self.operator.get();

        let mut output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, _pixel) in Pixels::new(input.surface(), bounds) {
                // Compute the kernel rectangle bounds.
                let kernel_bounds = IRect {
                    x0: (f64::from(x) - rx).floor() as i32,
                    y0: (f64::from(y) - ry).floor() as i32,
                    x1: (f64::from(x) + rx).ceil() as i32 + 1,
                    y1: (f64::from(y) + ry).ceil() as i32 + 1,
                };

                // Compute the new pixel values.
                let initial = match operator {
                    Operator::Erode => u8::max_value(),
                    Operator::Dilate => u8::min_value(),
                };

                let mut output_pixel = Pixel {
                    r: initial,
                    g: initial,
                    b: initial,
                    a: initial,
                };

                for (_x, _y, pixel) in
                    PixelRectangle::new(&input.surface(), bounds, kernel_bounds, EdgeMode::None)
                {
                    let op = match operator {
                        Operator::Erode => min,
                        Operator::Dilate => max,
                    };

                    output_pixel.r = op(output_pixel.r, pixel.r);
                    output_pixel.g = op(output_pixel.g, pixel.g);
                    output_pixel.b = op(output_pixel.b, pixel.b);
                    output_pixel.a = op(output_pixel.a, pixel.a);
                }

                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, input.surface().surface_type())?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}

impl Operator {
    fn parse(attr: Attribute, s: &str) -> Result<Self, NodeError> {
        match s {
            "erode" => Ok(Operator::Erode),
            "dilate" => Ok(Operator::Dilate),
            _ => Err(NodeError::parse_error(
                attr,
                ParseError::new("invalid value"),
            )),
        }
    }
}
