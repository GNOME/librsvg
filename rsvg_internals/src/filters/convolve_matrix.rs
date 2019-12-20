use cairo::{self, ImageSurface};
use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use nalgebra::{DMatrix, Dynamic, VecStorage};

use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::number_list::{NumberList, NumberListLength};
use crate::parsers::{NumberOptionalNumber, Parse, ParseValue, ParseValueToParseError};
use crate::property_bag::PropertyBag;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::SharedImageSurface,
    EdgeMode, ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// The `feConvolveMatrix` filter primitive.
pub struct FeConvolveMatrix {
    base: PrimitiveWithInput,
    order: (u32, u32),
    kernel_matrix: Option<DMatrix<f64>>,
    divisor: Option<f64>,
    bias: f64,
    target_x: Option<u32>,
    target_y: Option<u32>,
    edge_mode: EdgeMode,
    kernel_unit_length: Option<(f64, f64)>,
    preserve_alpha: bool,
}

impl Default for FeConvolveMatrix {
    /// Constructs a new `ConvolveMatrix` with empty properties.
    #[inline]
    fn default() -> FeConvolveMatrix {
        FeConvolveMatrix {
            base: PrimitiveWithInput::new::<Self>(),
            order: (3, 3),
            kernel_matrix: None,
            divisor: None,
            bias: 0.0,
            target_x: None,
            target_y: None,
            edge_mode: EdgeMode::Duplicate,
            kernel_unit_length: None,
            preserve_alpha: false,
        }
    }
}

impl NodeTrait for FeConvolveMatrix {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "order") => {
                    let NumberOptionalNumber(x, y) = attr.parse_to_parse_error_and_validate(
                        value,
                        |v: NumberOptionalNumber<i32>| {
                            if v.0 > 0 && v.1 > 0 {
                                Ok(v)
                            } else {
                                Err(ValueErrorKind::value_error("values must be greater than 0"))?
                            }
                        },
                    )?;
                    self.order = (x as u32, y as u32);
                }
                expanded_name!(svg "divisor") => {
                    self.divisor = Some(attr.parse_to_parse_error_and_validate(value, |x| {
                        if x != 0.0 {
                            Ok(x)
                        } else {
                            Err(ValueErrorKind::value_error("divisor cannot be equal to 0"))
                        }
                    })?)
                }
                expanded_name!(svg "bias") => self.bias = attr.parse(value)?,
                expanded_name!(svg "edgeMode") => self.edge_mode = attr.parse(value)?,
                expanded_name!(svg "kernelUnitLength") => {
                    let NumberOptionalNumber(x, y) =
                        attr.parse_to_parse_error_and_validate(value, |v: NumberOptionalNumber<f64>| {
                            if v.0 > 0.0 && v.1 > 0.0 {
                                Ok(v)
                            } else {
                                Err(ValueErrorKind::value_error(
                                    "kernelUnitLength can't be less or equal to zero",
                                ))
                            }
                        })?;

                    self.kernel_unit_length = Some((x, y))
                }
                expanded_name!(svg "preserveAlpha") => self.preserve_alpha = attr.parse(value)?,

                _ => (),
            }
        }

        // target_x and target_y depend on order.
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "targetX") => {
                    self.target_x = {
                        let v = attr.parse_to_parse_error_and_validate(value, |v: i32| {
                            if v >= 0 && v < self.order.0 as i32 {
                                Ok(v)
                            } else {
                                Err(ValueErrorKind::value_error(
                                    "targetX must be greater or equal to zero and less than orderX",
                                ))
                            }
                        })?;
                        Some(v as u32)
                    }
                }
                expanded_name!(svg "targetY") => {
                    self.target_y = {
                        let v = attr.parse_to_parse_error_and_validate(value, |v: i32| {
                            if v >= 0 && v < self.order.1 as i32 {
                                Ok(v)
                            } else {
                                Err(ValueErrorKind::value_error(
                                    "targetY must be greater or equal to zero and less than orderY",
                                ))
                            }
                        })?;
                        Some(v as u32)
                    }
                }
                _ => (),
            }
        }

        // Default values for target_x and target_y.
        if self.target_x.is_none() {
            self.target_x = Some(self.order.0 / 2);
        }
        if self.target_y.is_none() {
            self.target_y = Some(self.order.1 / 2);
        }

        // Finally, parse the kernel matrix.
        for (attr, value) in pbag
            .iter()
            .filter(|(attr, _)| attr.expanded() == expanded_name!(svg "kernelMatrix"))
        {
            self.kernel_matrix = Some({
                let number_of_elements = self.order.0 as usize * self.order.1 as usize;

                // #352: Parse as an unbounded list rather than exact length to prevent aborts due
                //       to huge allocation attempts by underlying Vec::with_capacity().
                let NumberList(v) =
                    NumberList::parse_str_to_parse_error(value, NumberListLength::Unbounded)
                        .attribute(attr.clone())?;

                if v.len() != number_of_elements {
                    return Err(ValueErrorKind::value_error(&format!(
                        "incorrect number of elements: expected {}",
                        number_of_elements
                    )))
                    .attribute(attr);
                }

                DMatrix::from_data(VecStorage::new(
                    Dynamic::new(self.order.1 as usize),
                    Dynamic::new(self.order.0 as usize),
                    v,
                ))
            });
        }

        // kernel_matrix must have been specified.
        if self.kernel_matrix.is_none() {
            return Err(ValueErrorKind::value_error("the value must be set"))
                .attribute(QualName::new(None, ns!(svg), local_name!("kernelMatrix")));
        }

        // Default value for the divisor.
        if self.divisor.is_none() {
            self.divisor = Some(self.kernel_matrix.as_ref().unwrap().iter().sum());

            if self.divisor.unwrap() == 0.0 {
                self.divisor = Some(1.0);
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeConvolveMatrix {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let mut bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);
        let original_bounds = bounds;

        let mut input_surface = if self.preserve_alpha {
            // preserve_alpha means we need to premultiply and unpremultiply the values.
            input.surface().unpremultiply(bounds)?
        } else {
            input.surface().clone()
        };

        let scale = self
            .kernel_unit_length
            .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

        if let Some((ox, oy)) = scale {
            // Scale the input surface to match kernel_unit_length.
            let (new_surface, new_bounds) = input_surface.scale(bounds, 1.0 / ox, 1.0 / oy)?;

            input_surface = new_surface;
            bounds = new_bounds;
        }

        let matrix = self.kernel_matrix.as_ref().unwrap();

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
                let kernel_bounds = IRect::new(
                    x as i32 - self.target_x.unwrap() as i32,
                    y as i32 - self.target_y.unwrap() as i32,
                    x as i32 - self.target_x.unwrap() as i32 + self.order.0 as i32,
                    y as i32 - self.target_y.unwrap() as i32 + self.order.1 as i32,
                );

                // Do the convolution.
                let mut r = 0.0;
                let mut g = 0.0;
                let mut b = 0.0;
                let mut a = 0.0;

                for (x, y, pixel) in
                    PixelRectangle::new(&input_surface, bounds, kernel_bounds, self.edge_mode)
                {
                    let kernel_x = (kernel_bounds.x1 - x - 1) as usize;
                    let kernel_y = (kernel_bounds.y1 - y - 1) as usize;

                    r += f64::from(pixel.r) / 255.0 * matrix[(kernel_y, kernel_x)];
                    g += f64::from(pixel.g) / 255.0 * matrix[(kernel_y, kernel_x)];
                    b += f64::from(pixel.b) / 255.0 * matrix[(kernel_y, kernel_x)];

                    if !self.preserve_alpha {
                        a += f64::from(pixel.a) / 255.0 * matrix[(kernel_y, kernel_x)];
                    }
                }

                // If preserve_alpha is true, set a to the source alpha value.
                if self.preserve_alpha {
                    a = f64::from(pixel.a) / 255.0;
                } else {
                    a = a / self.divisor.unwrap() + self.bias;
                }

                let clamped_a = clamp(a, 0.0, 1.0);

                let compute = |x| {
                    let x = x / self.divisor.unwrap() + self.bias * a;

                    let x = if self.preserve_alpha {
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
            name: self.base.result.clone(),
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

impl Parse for EdgeMode {
    fn parse(parser: &mut Parser<'_, '_>) -> Result<Self, ValueErrorKind> {
        parse_identifiers!(
            parser,
            "duplicate" => EdgeMode::Duplicate,
            "wrap" => EdgeMode::Wrap,
            "none" => EdgeMode::None,
        )
        .map_err(|_| ValueErrorKind::parse_error("parse error"))
    }
}

// Used for the preserveAlpha attribute
impl Parse for bool {
    fn parse(parser: &mut Parser<'_, '_>) -> Result<Self, ValueErrorKind> {
        parse_identifiers!(
            parser,
            "false" => false,
            "true" => true,
        )
        .map_err(|_| ValueErrorKind::parse_error("parse error"))
    }
}
