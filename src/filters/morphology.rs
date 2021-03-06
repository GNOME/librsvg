use std::cmp::{max, min};

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::parsers::{NonNegative, NumberOptionalNumber, Parse, ParseValue};
use crate::property_defs::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::ExclusiveImageSurface,
    EdgeMode, ImageSurfaceDataExt, Pixel,
};
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams};

/// Enumeration of the possible morphology operations.
#[derive(Clone)]
enum Operator {
    Erode,
    Dilate,
}

/// The `feMorphology` filter primitive.
#[derive(Clone)]
pub struct FeMorphology {
    base: Primitive,
    in1: Input,
    operator: Operator,
    radius: (f64, f64),
}

/// Resolved `feMorphology` primitive for rendering.
pub type Morphology = FeMorphology;

impl Default for FeMorphology {
    /// Constructs a new `Morphology` with empty properties.
    #[inline]
    fn default() -> FeMorphology {
        FeMorphology {
            base: Primitive::new(),
            in1: Default::default(),
            operator: Operator::Erode,
            radius: (0.0, 0.0),
        }
    }
}

impl SetAttributes for FeMorphology {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.in1 = self.base.parse_one_input(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "operator") => self.operator = attr.parse(value)?,
                expanded_name!("", "radius") => {
                    let NumberOptionalNumber(NonNegative(x), NonNegative(y)) = attr.parse(value)?;
                    self.radius = (x, y);
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl FeMorphology {
    pub fn render(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        // Although https://www.w3.org/TR/filter-effects/#propdef-color-interpolation-filters does not mention
        // feMorphology as being one of the primitives that does *not* use that property,
        // the SVG1.1 test for filters-morph-01-f.svg fails if we pass the value from the ComputedValues here (that
        // document does not specify the color-interpolation-filters property, so it defaults to linearRGB).
        // So, we pass Auto, which will get resolved to SRGB, and that makes that test pass.
        //
        // I suppose erosion/dilation doesn't care about the color space of the source image?

        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            ColorInterpolationFilters::Auto,
        )?;
        let bounds = self
            .base
            .get_bounds(ctx)?
            .add_input(&input_1)
            .into_irect(ctx, draw_ctx);

        let (rx, ry) = self.radius;
        let (rx, ry) = ctx.paffine().transform_distance(rx, ry);

        // The radii can become negative here due to the transform.
        let (rx, ry) = (rx.abs(), ry.abs());

        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input_1.surface().surface_type(),
        )?;

        surface.modify(&mut |data, stride| {
            for (x, y, _pixel) in Pixels::within(input_1.surface(), bounds) {
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

                for (_x, _y, pixel) in PixelRectangle::within(
                    &input_1.surface(),
                    bounds,
                    kernel_bounds,
                    EdgeMode::None,
                ) {
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
}

impl FilterEffect for FeMorphology {
    fn resolve(&self, _node: &Node) -> Result<PrimitiveParams, FilterError> {
        Ok(PrimitiveParams::Morphology(self.clone()))
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
