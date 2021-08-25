use std::cmp::{max, min};

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::parsers::{NonNegative, NumberOptionalNumber, Parse, ParseValue};
use crate::properties::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::ExclusiveImageSurface,
    EdgeMode, ImageSurfaceDataExt, Pixel,
};
use crate::xml::Attributes;

use super::bounds::BoundsBuilder;
use super::context::{FilterContext, FilterOutput};
use super::{
    FilterEffect, FilterError, FilterResolveError, Input, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};

/// Enumeration of the possible morphology operations.
#[derive(Clone)]
enum Operator {
    Erode,
    Dilate,
}

enum_default!(Operator, Operator::Erode);

/// The `feMorphology` filter primitive.
#[derive(Default)]
pub struct FeMorphology {
    base: Primitive,
    params: Morphology,
}

/// Resolved `feMorphology` primitive for rendering.
#[derive(Clone, Default)]
pub struct Morphology {
    in1: Input,
    operator: Operator,
    radius: (f64, f64),
}

impl SetAttributes for FeMorphology {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.params.in1 = self.base.parse_one_input(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "operator") => self.params.operator = attr.parse(value)?,
                expanded_name!("", "radius") => {
                    let NumberOptionalNumber(NonNegative(x), NonNegative(y)) = attr.parse(value)?;
                    self.params.radius = (x, y);
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Morphology {
    pub fn render(
        &self,
        bounds_builder: BoundsBuilder,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterOutput, FilterError> {
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
        let bounds: IRect = bounds_builder
            .add_input(&input_1)
            .compute(ctx)
            .clipped
            .into();

        let (rx, ry) = self.radius;
        let (rx, ry) = ctx.paffine().transform_distance(rx, ry);

        // The radii can become negative here due to the transform.
        // Additionally The radii being excessively large causes cpu hangups
        let (rx, ry) = (rx.abs().min(10.0), ry.abs().min(10.0));

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

                for (_x, _y, pixel) in
                    PixelRectangle::within(input_1.surface(), bounds, kernel_bounds, EdgeMode::None)
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

        Ok(FilterOutput {
            surface: surface.share()?,
            bounds,
        })
    }
}

impl FilterEffect for FeMorphology {
    fn resolve(
        &self,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        _node: &Node,
    ) -> Result<ResolvedPrimitive, FilterResolveError> {
        Ok(ResolvedPrimitive {
            primitive: self.base.clone(),
            params: PrimitiveParams::Morphology(self.params.clone()),
        })
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
