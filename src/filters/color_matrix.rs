use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use nalgebra::{Matrix3, Matrix4x5, Matrix5, Vector5};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{NumberList, NumberListLength, Parse, ParseValue};
use crate::property_defs::ColorInterpolationFilters;
use crate::surface_utils::{
    iterators::Pixels, shared_surface::ExclusiveImageSurface, ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams, ResolvedPrimitive};

/// Color matrix operation types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OperationType {
    Matrix,
    Saturate,
    HueRotate,
    LuminanceToAlpha,
}

enum_default!(OperationType, OperationType::Matrix);

/// The `feColorMatrix` filter primitive.
#[derive(Default)]
pub struct FeColorMatrix {
    base: Primitive,
    params: ColorMatrix,
}

/// Resolved `feColorMatrix` primitive for rendering.
#[derive(Clone)]
pub struct ColorMatrix {
    in1: Input,
    matrix: Matrix5<f64>,
    color_interpolation_filters: ColorInterpolationFilters,
}

impl Default for ColorMatrix {
    fn default() -> ColorMatrix {
        ColorMatrix {
            in1: Default::default(),
            color_interpolation_filters: Default::default(),

            // nalgebra's Default for Matrix5 is all zeroes, so we actually need this :(
            matrix: Matrix5::identity(),
        }
    }
}

#[rustfmt::skip]
impl SetAttributes for FeColorMatrix {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.params.in1 = self.base.parse_one_input(attrs)?;

        // First, determine the operation type.
        let mut operation_type = Default::default();
        for (attr, value) in attrs
            .iter()
            .filter(|(attr, _)| attr.expanded() == expanded_name!("", "type"))
        {
            operation_type = attr.parse(value)?;
        }

        // Now read the matrix correspondingly.
        // LuminanceToAlpha doesn't accept any matrix.
        if operation_type == OperationType::LuminanceToAlpha {
            self.params.matrix = {
                Matrix5::new(
                    0.0,    0.0,    0.0,    0.0, 0.0,
                    0.0,    0.0,    0.0,    0.0, 0.0,
                    0.0,    0.0,    0.0,    0.0, 0.0,
                    0.2125, 0.7154, 0.0721, 0.0, 0.0,
                    0.0,    0.0,    0.0,    0.0, 1.0,
                )
            };
        } else {
            for (attr, value) in attrs
                .iter()
                .filter(|(attr, _)| attr.expanded() == expanded_name!("", "values"))
            {
                let new_matrix = match operation_type {
                    OperationType::LuminanceToAlpha => unreachable!(),
                    OperationType::Matrix => {
                        let NumberList(v) =
                            NumberList::parse_str(value, NumberListLength::Exact(20))
                                .attribute(attr)?;
                        let matrix = Matrix4x5::from_row_slice(&v);
                        let mut matrix = matrix.fixed_resize(0.0);
                        matrix[(4, 4)] = 1.0;
                        matrix
                    }
                    OperationType::Saturate => {
                        let s: f64 = attr.parse(value)?;

                        Matrix5::new(
                            0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s, 0.0, 0.0,
                            0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s, 0.0, 0.0,
                            0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s, 0.0, 0.0,
                            0.0,               0.0,               0.0,               1.0, 0.0,
                            0.0,               0.0,               0.0,               0.0, 1.0,
                        )
                    }
                    OperationType::HueRotate => {
                        let degrees: f64 = attr.parse(value)?;
                        let (sin, cos) = degrees.to_radians().sin_cos();

                        let a = Matrix3::new(
                            0.213, 0.715, 0.072,
                            0.213, 0.715, 0.072,
                            0.213, 0.715, 0.072,
                        );

                        let b = Matrix3::new(
                             0.787, -0.715, -0.072,
                            -0.213,  0.285, -0.072,
                            -0.213, -0.715,  0.928,
                        );

                        let c = Matrix3::new(
                            -0.213, -0.715,  0.928,
                             0.143,  0.140, -0.283,
                            -0.787,  0.715,  0.072,
                        );

                        let top_left = a + b * cos + c * sin;

                        let mut matrix = top_left.fixed_resize(0.0);
                        matrix[(3, 3)] = 1.0;
                        matrix[(4, 4)] = 1.0;
                        matrix
                    }
                };

                self.params.matrix = new_matrix;
            }
        }

        Ok(())
    }
}

impl ColorMatrix {
    pub fn render(
        &self,
        primitive: &ResolvedPrimitive,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            self.color_interpolation_filters,
        )?;
        let bounds = primitive
            .get_bounds(ctx)?
            .add_input(&input_1)
            .into_irect(ctx);

        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input_1.surface().surface_type(),
        )?;

        surface.modify(&mut |data, stride| {
            for (x, y, pixel) in Pixels::within(input_1.surface(), bounds) {
                let alpha = f64::from(pixel.a) / 255f64;

                let pixel_vec = if alpha == 0.0 {
                    Vector5::new(0.0, 0.0, 0.0, 0.0, 1.0)
                } else {
                    Vector5::new(
                        f64::from(pixel.r) / 255f64 / alpha,
                        f64::from(pixel.g) / 255f64 / alpha,
                        f64::from(pixel.b) / 255f64 / alpha,
                        alpha,
                        1.0,
                    )
                };
                let mut new_pixel_vec = Vector5::zeros();
                self.matrix.mul_to(&pixel_vec, &mut new_pixel_vec);

                let new_alpha = clamp(new_pixel_vec[3], 0.0, 1.0);

                let premultiply = |x: f64| ((clamp(x, 0.0, 1.0) * new_alpha * 255f64) + 0.5) as u8;

                let output_pixel = Pixel {
                    r: premultiply(new_pixel_vec[0]),
                    g: premultiply(new_pixel_vec[1]),
                    b: premultiply(new_pixel_vec[2]),
                    a: ((new_alpha * 255f64) + 0.5) as u8,
                };

                data.set_pixel(stride, output_pixel, x, y);
            }
        });

        Ok(FilterResult {
            name: primitive.result.clone(),
            output: FilterOutput {
                surface: surface.share()?,
                bounds,
            },
        })
    }
}

impl FilterEffect for FeColorMatrix {
    fn resolve(&self, node: &Node) -> Result<(Primitive, PrimitiveParams), FilterError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        let mut params = self.params.clone();
        params.color_interpolation_filters = values.color_interpolation_filters();

        Ok((self.base.clone(), PrimitiveParams::ColorMatrix(params)))
    }
}

impl Parse for OperationType {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "matrix" => OperationType::Matrix,
            "saturate" => OperationType::Saturate,
            "hueRotate" => OperationType::HueRotate,
            "luminanceToAlpha" => OperationType::LuminanceToAlpha,
        )?)
    }
}
