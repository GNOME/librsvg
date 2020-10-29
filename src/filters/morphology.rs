use std::cmp::{max, min};

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::parsers::{NumberOptionalNumber, Parse, ParseValue};
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::ExclusiveImageSurface,
    EdgeMode, ImageSurfaceDataExt, Pixel,
};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// Enumeration of the possible morphology operations.
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

impl SetAttributes for FeMorphology {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "operator") => self.operator = attr.parse(value)?,
                expanded_name!("", "radius") => {
                    let NumberOptionalNumber(x, y) =
                        attr.parse_and_validate(value, |v: NumberOptionalNumber<f64>| {
                            if v.0 >= 0.0 && v.1 >= 0.0 {
                                Ok(v)
                            } else {
                                Err(ValueErrorKind::value_error("radius cannot be negative"))
                            }
                        })?;

                    self.radius = (x, y);
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
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, acquired_nodes, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx, node.parent().as_ref())?
            .add_input(&input)
            .into_irect(draw_ctx);

        let (rx, ry) = self.radius;
        let (rx, ry) = ctx.paffine().transform_distance(rx, ry);

        // The radii can become negative here due to the transform.
        let (rx, ry) = (rx.abs(), ry.abs());

        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input.surface().surface_type(),
        )?;

        surface.modify(&mut |data, stride| {
            for (x, y, _pixel) in Pixels::within(input.surface(), bounds) {
                // Compute the kernel rectangle bounds.
                let kernel_bounds = IRect::new(
                    (f64::from(x) - rx).floor() as i32,
                    (f64::from(y) - ry).floor() as i32,
                    (f64::from(x) + rx).ceil() as i32 + 1,
                    (f64::from(y) + ry).ceil() as i32 + 1,
                );

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
                    PixelRectangle::within(&input.surface(), bounds, kernel_bounds, EdgeMode::None)
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

                data.set_pixel(stride, output_pixel, x, y);
            }
        });

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: surface.share()?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}

impl Parse for Operator {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "erode" => Operator::Erode,
            "dilate" => Operator::Dilate,
        )?)
    }
}
