use std::cmp::{max, min};

use cairo::{self, ImageSurface};
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};

use crate::drawing_ctx::DrawingCtx;
use crate::error::{AttributeResultExt, NodeError};
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{self, ParseError};
use crate::property_bag::PropertyBag;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::SharedImageSurface,
    EdgeMode,
    ImageSurfaceDataExt,
    Pixel,
};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// Enumeration of the possible morphology operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum Operator {
    Erode,
    Dilate,
}

/// The `feMorphology` filter primitive.
pub struct FeMorphology {
    base: PrimitiveWithInput,
    operator: Operator,
    radius: (f64, f64),
}

impl Default for FeMorphology {
    /// Constructs a new `Morphology` with empty properties.
    #[inline]
    fn default() -> FeMorphology {
        FeMorphology {
            base: PrimitiveWithInput::new::<Self>(),
            operator: Operator::Erode,
            radius: (0.0, 0.0),
        }
    }
}

impl NodeTrait for FeMorphology {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "operator") => self.operator = Operator::parse(attr, value)?,
                expanded_name!(svg "radius") => {
                    self.radius = parsers::number_optional_number(value)
                        .attribute(attr.clone())
                        .and_then(|(x, y)| {
                            if x >= 0.0 && y >= 0.0 {
                                Ok((x, y))
                            } else {
                                Err(NodeError::value_error(attr, "radius cannot be negative"))
                            }
                        })?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeMorphology {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);

        let (rx, ry) = self.radius;
        let (rx, ry) = ctx.paffine().transform_distance(rx, ry);

        // The radii can become negative here due to the transform.
        let rx = rx.abs();
        let ry = ry.abs();

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
                let initial = match self.operator {
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
                    let op = match self.operator {
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
            name: self.base.result.clone(),
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
    fn parse(attr: QualName, s: &str) -> Result<Self, NodeError> {
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
