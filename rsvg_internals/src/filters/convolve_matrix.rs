use std::cell::{Cell, RefCell};

use cairo::{self, ImageSurface, MatrixTrait};
use nalgebra::{DMatrix, Dynamic, MatrixVec};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{self, ListLength, NumberListError, ParseError};
use property_bag::PropertyBag;
use surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::SharedImageSurface,
    EdgeMode,
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::{Filter, FilterError, PrimitiveWithInput};

/// The `feConvolveMatrix` filter primitive.
pub struct ConvolveMatrix {
    base: PrimitiveWithInput,
    order: Cell<(u32, u32)>,
    kernel_matrix: RefCell<Option<DMatrix<f64>>>,
    divisor: Cell<Option<f64>>,
    bias: Cell<f64>,
    target_x: Cell<Option<u32>>,
    target_y: Cell<Option<u32>>,
    edge_mode: Cell<EdgeMode>,
    kernel_unit_length: Cell<Option<(f64, f64)>>,
    preserve_alpha: Cell<bool>,
}

impl ConvolveMatrix {
    /// Constructs a new `ConvolveMatrix` with empty properties.
    #[inline]
    pub fn new() -> ConvolveMatrix {
        ConvolveMatrix {
            base: PrimitiveWithInput::new::<Self>(),
            order: Cell::new((3, 3)),
            kernel_matrix: RefCell::new(None),
            divisor: Cell::new(None),
            bias: Cell::new(0.0),
            target_x: Cell::new(None),
            target_y: Cell::new(None),
            edge_mode: Cell::new(EdgeMode::Duplicate),
            kernel_unit_length: Cell::new(None),
            preserve_alpha: Cell::new(false),
        }
    }
}

impl NodeTrait for ConvolveMatrix {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Order => self.order.set(
                    parsers::integer_optional_integer(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|(x, y)| {
                            if x > 0 && y > 0 {
                                Ok((x as u32, y as u32))
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "values must be greater than 0",
                                ))
                            }
                        })?,
                ),
                Attribute::Divisor => self.divisor.set(Some(
                    parsers::number(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|x| {
                            if x != 0.0 {
                                Ok(x)
                            } else {
                                Err(NodeError::value_error(attr, "divisor cannot be equal to 0"))
                            }
                        })?,
                )),
                Attribute::Bias => self.bias.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                Attribute::EdgeMode => self.edge_mode.set(EdgeMode::parse(attr, value)?),
                Attribute::KernelUnitLength => self.kernel_unit_length.set(Some(
                    parsers::number_optional_number(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|(x, y)| {
                            if x > 0.0 && y > 0.0 {
                                Ok((x, y))
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "kernelUnitLength can't be less or equal to zero",
                                ))
                            }
                        })?,
                )),
                Attribute::PreserveAlpha => self.preserve_alpha.set(match value {
                    "false" => false,
                    "true" => true,
                    _ => {
                        return Err(NodeError::parse_error(
                            attr,
                            ParseError::new("expected false or true"),
                        ));
                    }
                }),
                _ => (),
            }
        }

        // target_x and target_y depend on order.
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::TargetX => self.target_x.set(Some(
                    parsers::integer(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|x| {
                            if x >= 0 && x < self.order.get().0 as i32 {
                                Ok(x as u32)
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "targetX must be greater or equal to zero and less than orderX",
                                ))
                            }
                        })?,
                )),
                Attribute::TargetY => self.target_y.set(Some(
                    parsers::integer(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|x| {
                            if x >= 0 && x < self.order.get().1 as i32 {
                                Ok(x as u32)
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "targetY must be greater or equal to zero and less than orderY",
                                ))
                            }
                        })?,
                )),
                _ => (),
            }
        }

        // Default values for target_x and target_y.
        if self.target_x.get().is_none() {
            self.target_x.set(Some(self.order.get().0 / 2));
        }
        if self.target_y.get().is_none() {
            self.target_y.set(Some(self.order.get().1 / 2));
        }

        // Finally, parse the kernel matrix.
        for (_, attr, value) in pbag
            .iter()
            .filter(|(_, attr, _)| *attr == Attribute::KernelMatrix)
        {
            self.kernel_matrix.replace(Some({
                let number_of_elements = self.order.get().0 as usize * self.order.get().1 as usize;

                // #352: Parse as an unbounded list rather than exact length to prevent aborts due
                //       to huge allocation attempts by underlying Vec::with_capacity().
                let elements = parsers::number_list_from_str(value, ListLength::Unbounded)
                    .map_err(|err| {
                        NodeError::parse_error(
                            attr,
                            match err {
                                NumberListError::IncorrectNumberOfElements => unreachable!(),
                                NumberListError::Parse(err) => err,
                            },
                        )
                    })?;

                if elements.len() != number_of_elements {
                    return Err(NodeError::value_error(
                        attr,
                        &format!(
                            "incorrect number of elements: expected {}",
                            number_of_elements
                        ),
                    ));
                }

                DMatrix::from_data(MatrixVec::new(
                    Dynamic::new(self.order.get().1 as usize),
                    Dynamic::new(self.order.get().0 as usize),
                    elements,
                ))
            }));
        }

        // kernel_matrix must have been specified.
        if self.kernel_matrix.borrow().is_none() {
            return Err(NodeError::value_error(
                Attribute::KernelMatrix,
                "the value must be set",
            ));
        }

        // Default value for the divisor.
        if self.divisor.get().is_none() {
            self.divisor.set(Some(
                self.kernel_matrix.borrow().as_ref().unwrap().iter().sum(),
            ));

            if self.divisor.get().unwrap() == 0.0 {
                self.divisor.set(Some(1.0));
            }
        }

        Ok(())
    }
}

impl Filter for ConvolveMatrix {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let mut bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);
        let original_bounds = bounds;

        let mut input_surface = if self.preserve_alpha.get() {
            // preserve_alpha means we need to premultiply and unpremultiply the values.
            input.surface().unpremultiply(bounds)?
        } else {
            input.surface().clone()
        };

        let scale = self
            .kernel_unit_length
            .get()
            .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

        if let Some((ox, oy)) = scale {
            // Scale the input surface to match kernel_unit_length.
            let (new_surface, new_bounds) = input_surface.scale(bounds, 1.0 / ox, 1.0 / oy)?;

            input_surface = new_surface;
            bounds = new_bounds;
        }

        let matrix = self.kernel_matrix.borrow();
        let matrix = matrix.as_ref().unwrap();

        let mut output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            input_surface.width(),
            input_surface.height(),
        )?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, pixel) in Pixels::new(&input_surface, bounds) {
                // Compute the convolution rectangle bounds.
                let kernel_bounds = IRect {
                    x0: x as i32 - self.target_x.get().unwrap() as i32,
                    y0: y as i32 - self.target_y.get().unwrap() as i32,
                    x1: x as i32 - self.target_x.get().unwrap() as i32 + self.order.get().0 as i32,
                    y1: y as i32 - self.target_y.get().unwrap() as i32 + self.order.get().1 as i32,
                };

                // Do the convolution.
                let mut r = 0.0;
                let mut g = 0.0;
                let mut b = 0.0;
                let mut a = 0.0;

                for (x, y, pixel) in
                    PixelRectangle::new(&input_surface, bounds, kernel_bounds, self.edge_mode.get())
                {
                    let kernel_x = (kernel_bounds.x1 - x - 1) as usize;
                    let kernel_y = (kernel_bounds.y1 - y - 1) as usize;

                    r += f64::from(pixel.r) / 255.0 * matrix[(kernel_y, kernel_x)];
                    g += f64::from(pixel.g) / 255.0 * matrix[(kernel_y, kernel_x)];
                    b += f64::from(pixel.b) / 255.0 * matrix[(kernel_y, kernel_x)];

                    if !self.preserve_alpha.get() {
                        a += f64::from(pixel.a) / 255.0 * matrix[(kernel_y, kernel_x)];
                    }
                }

                // If preserve_alpha is true, set a to the source alpha value.
                if self.preserve_alpha.get() {
                    a = f64::from(pixel.a) / 255.0;
                } else {
                    a = a / self.divisor.get().unwrap() + self.bias.get();
                }

                let clamped_a = clamp(a, 0.0, 1.0);

                let compute = |x| {
                    let x = x / self.divisor.get().unwrap() + self.bias.get() * a;

                    let x = if self.preserve_alpha.get() {
                        // Premultiply the output value.
                        clamp(x, 0.0, 1.0) * clamped_a
                    } else {
                        clamp(x, 0.0, clamped_a)
                    };

                    ((x * 255.0) + 0.5) as u8
                };

                let output_pixel = Pixel {
                    r: compute(r),
                    g: compute(g),
                    b: compute(b),
                    a: ((clamped_a * 255.0) + 0.5) as u8,
                };

                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        let mut output_surface =
            SharedImageSurface::new(output_surface, input.surface().surface_type())?;

        if let Some((ox, oy)) = scale {
            // Scale the output surface back.
            output_surface = output_surface.scale_to(
                ctx.source_graphic().width(),
                ctx.source_graphic().height(),
                original_bounds,
                ox,
                oy,
            )?;

            bounds = original_bounds;
        }

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: output_surface,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}

impl EdgeMode {
    fn parse(attr: Attribute, s: &str) -> Result<Self, NodeError> {
        match s {
            "duplicate" => Ok(EdgeMode::Duplicate),
            "wrap" => Ok(EdgeMode::Wrap),
            "none" => Ok(EdgeMode::None),
            _ => Err(NodeError::parse_error(
                attr,
                ParseError::new("invalid value"),
            )),
        }
    }
}
