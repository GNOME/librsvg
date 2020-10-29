use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use nalgebra::{Matrix3, Matrix4x5, Matrix5, Vector5};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::number_list::{NumberList, NumberListLength};
use crate::parsers::{Parse, ParseValue};
use crate::surface_utils::{
    iterators::Pixels, shared_surface::ExclusiveImageSurface, ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// Color matrix operation types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OperationType {
    Matrix,
    Saturate,
    HueRotate,
    LuminanceToAlpha,
}

impl Default for OperationType {
    fn default() -> Self {
        OperationType::Matrix
    }
}

/// The `feColorMatrix` filter primitive.
pub struct FeColorMatrix {
    base: PrimitiveWithInput,
    matrix: Matrix5<f64>,
}

impl Default for FeColorMatrix {
    /// Constructs a new `ColorMatrix` with empty properties.
    #[inline]
    fn default() -> FeColorMatrix {
        FeColorMatrix {
            base: PrimitiveWithInput::new::<Self>(),
            matrix: Matrix5::identity(),
        }
    }
}

#[rustfmt::skip]
impl SetAttributes for FeColorMatrix {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

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
            self.matrix = {
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
                        let s: f64 = attr.parse_and_validate(value, |s| {
                            if s < 0.0 || s > 1.0 {
                                Err(ValueErrorKind::value_error("expected value from 0 to 1"))
                            } else {
                                Ok(s)
                            }
                        })?;

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

                self.matrix = new_matrix;
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeColorMatrix {
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

        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input.surface().surface_type(),
        )?;

        surface.modify(&mut |data, stride| {
            for (x, y, pixel) in Pixels::within(input.surface(), bounds) {
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
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: surface.share()?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
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
