use std::cell::RefCell;

use cairo::{self, ImageSurface};
use rulinalg::{
    self,
    matrix::{BaseMatrix, BaseMatrixMut, Matrix},
};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{self, ListLength, NumberListError, ParseError};
use property_bag::PropertyBag;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{Filter, FilterError, PrimitiveWithInput};

/// Color matrix operation types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OperationType {
    Matrix,
    Saturate,
    HueRotate,
    LuminanceToAlpha,
}

/// The `feColorMatrix` filter primitive.
pub struct ColorMatrix {
    base: PrimitiveWithInput,
    matrix: RefCell<Matrix<f64>>,
}

impl ColorMatrix {
    /// Constructs a new `ColorMatrix` with empty properties.
    #[inline]
    pub fn new() -> ColorMatrix {
        ColorMatrix {
            base: PrimitiveWithInput::new::<Self>(),
            matrix: RefCell::new(Matrix::identity(5)),
        }
    }
}

impl NodeTrait for ColorMatrix {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        // First, determine the operation type.
        let mut operation_type = OperationType::Matrix;
        for (_, attr, value) in pbag.iter().filter(|(_, attr, _)| *attr == Attribute::Type) {
            operation_type = OperationType::parse(attr, value)?;
        }

        // Now read the matrix correspondingly.
        // LuminanceToAlpha doesn't accept any matrix.
        if operation_type == OperationType::LuminanceToAlpha {
            self.matrix.replace(matrix![
                                0.0,    0.0,    0.0,    0.0, 0.0;
                                0.0,    0.0,    0.0,    0.0, 0.0;
                                0.0,    0.0,    0.0,    0.0, 0.0;
                                0.2125, 0.7154, 0.0721, 0.0, 0.0;
                                0.0,    0.0,    0.0,    0.0, 1.0
            ]);
        } else {
            for (_, attr, value) in pbag
                .iter()
                .filter(|(_, attr, _)| *attr == Attribute::Values)
            {
                let new_matrix = match operation_type {
                    OperationType::LuminanceToAlpha => unreachable!(),
                    OperationType::Matrix => {
                        let top = Matrix::new(
                            4,
                            5,
                            parsers::number_list_from_str(value, ListLength::Exact(20)).map_err(
                                |err| {
                                    NodeError::parse_error(
                                        attr,
                                        match err {
                                            NumberListError::IncorrectNumberOfElements => {
                                                ParseError::new(
                                                    "incorrect number of elements: expected 20",
                                                )
                                            }
                                            NumberListError::Parse(err) => err,
                                        },
                                    )
                                },
                            )?,
                        );

                        let mut matrix = Matrix::identity(5);
                        matrix.sub_slice_mut([0, 0], 4, 5).set_to(top);
                        matrix
                    }
                    OperationType::Saturate => {
                        let s = parsers::number(value)
                            .map_err(|err| NodeError::parse_error(attr, err))?;
                        if s < 0.0 || s > 1.0 {
                            return Err(NodeError::value_error(attr, "expected value from 0 to 1"));
                        }

                        matrix![
                            0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s, 0.0, 0.0;
                            0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s, 0.0, 0.0;
                            0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s, 0.0, 0.0;
                            0.0,               0.0,               0.0,               1.0, 0.0;
                            0.0,               0.0,               0.0,               0.0, 1.0
                        ]
                    }
                    OperationType::HueRotate => {
                        let degrees = parsers::number(value)
                            .map_err(|err| NodeError::parse_error(attr, err))?;

                        let (sin, cos) = degrees.to_radians().sin_cos();

                        let a = matrix![
                            0.213, 0.715, 0.072;
                            0.213, 0.715, 0.072;
                            0.213, 0.715, 0.072
                        ];

                        let b = matrix![
                             0.787, -0.715, -0.072;
                            -0.213,  0.285, -0.072;
                            -0.213, -0.715,  0.928
                        ];

                        let c = matrix![
                            -0.213, -0.715,  0.928;
                             0.143,  0.140, -0.283;
                            -0.787,  0.715,  0.072
                        ];

                        let top_left = a + b * cos + c * sin;

                        let mut matrix = Matrix::identity(5);
                        matrix.sub_slice_mut([0, 0], 3, 3).set_to(top_left);
                        matrix
                    }
                };

                self.matrix.replace(new_matrix);
            }
        }

        Ok(())
    }
}

impl Filter for ColorMatrix {
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

        let matrix = &*self.matrix.borrow();

        /// Multiplies a matrix by a vector and puts the result into a slice.
        #[inline]
        fn mul_into(out: &mut [f64], m: &Matrix<f64>, v: &[f64]) {
            assert_eq!(v.len(), m.cols());
            assert_eq!(v.len(), out.len());

            for (i, row) in m.row_iter().enumerate() {
                out[i] = rulinalg::utils::dot(row.raw_slice(), v);
            }
        }

        let mut output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        ).map_err(FilterError::IntermediateSurfaceCreation)?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, pixel) in Pixels::new(input.surface(), bounds) {
                let alpha = f64::from(pixel.a) / 255f64;

                let pixel_vec = if alpha == 0.0 {
                    [0.0, 0.0, 0.0, 0.0, 1.0]
                } else {
                    [
                        f64::from(pixel.r) / 255f64 / alpha,
                        f64::from(pixel.g) / 255f64 / alpha,
                        f64::from(pixel.b) / 255f64 / alpha,
                        alpha,
                        1.0,
                    ]
                };
                let mut new_pixel_vec = [0.0; 5];
                mul_into(&mut new_pixel_vec, &matrix, &pixel_vec);

                let new_alpha = clamp(new_pixel_vec[3], 0.0, 1.0);

                let premultiply = |x: f64| (clamp(x, 0.0, 1.0) * new_alpha * 255f64).round() as u8;

                let output_pixel = Pixel {
                    r: premultiply(new_pixel_vec[0]),
                    g: premultiply(new_pixel_vec[1]),
                    b: premultiply(new_pixel_vec[2]),
                    a: (new_alpha * 255f64).round() as u8,
                };

                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface)
                    .map_err(FilterError::BadIntermediateSurfaceStatus)?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}

impl OperationType {
    fn parse(attr: Attribute, s: &str) -> Result<Self, NodeError> {
        match s {
            "matrix" => Ok(OperationType::Matrix),
            "saturate" => Ok(OperationType::Saturate),
            "hueRotate" => Ok(OperationType::HueRotate),
            "luminanceToAlpha" => Ok(OperationType::LuminanceToAlpha),
            _ => Err(NodeError::parse_error(
                attr,
                ParseError::new("invalid value"),
            )),
        }
    }
}
