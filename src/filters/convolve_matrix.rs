use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use nalgebra::{DMatrix, Dynamic, VecStorage};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{
    NonNegative, NumberList, NumberListLength, NumberOptionalNumber, Parse, ParseValue,
};
use crate::property_defs::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::{PixelRectangle, Pixels},
    shared_surface::ExclusiveImageSurface,
    EdgeMode, ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;
use crate::xml::Attributes;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, Primitive, PrimitiveParams, ResolvedPrimitive};

/// The `feConvolveMatrix` filter primitive.
pub struct FeConvolveMatrix {
    base: Primitive,
    in1: Input,
    order: (u32, u32),
    kernel_matrix: Option<DMatrix<f64>>,
    divisor: f64,
    bias: f64,
    target_x: Option<u32>,
    target_y: Option<u32>,
    edge_mode: EdgeMode,
    kernel_unit_length: Option<(f64, f64)>,
    preserve_alpha: bool,
}

/// Resolved `feConvolveMatrix` primitive for rendering.
pub struct ConvolveMatrix {
    in1: Input,
    order: (u32, u32),
    kernel_matrix: Option<DMatrix<f64>>,
    divisor: f64,
    bias: f64,
    target_x: Option<u32>,
    target_y: Option<u32>,
    edge_mode: EdgeMode,
    kernel_unit_length: Option<(f64, f64)>,
    preserve_alpha: bool,
    color_interpolation_filters: ColorInterpolationFilters,
}

impl Default for FeConvolveMatrix {
    /// Constructs a new `ConvolveMatrix` with empty properties.
    #[inline]
    fn default() -> FeConvolveMatrix {
        FeConvolveMatrix {
            base: Primitive::new(),
            in1: Default::default(),
            order: (3, 3),
            kernel_matrix: None,
            divisor: 0.0,
            bias: 0.0,
            target_x: None,
            target_y: None,
            edge_mode: EdgeMode::Duplicate,
            kernel_unit_length: None,
            preserve_alpha: false,
        }
    }
}

impl SetAttributes for FeConvolveMatrix {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.in1 = self.base.parse_one_input(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "order") => {
                    let NumberOptionalNumber(x, y) = attr.parse(value)?;
                    self.order = (x, y);
                }
                expanded_name!("", "divisor") => self.divisor = attr.parse(value)?,
                expanded_name!("", "bias") => self.bias = attr.parse(value)?,
                expanded_name!("", "targetX") => self.target_x = attr.parse(value)?,
                expanded_name!("", "targetY") => self.target_y = attr.parse(value)?,
                expanded_name!("", "edgeMode") => self.edge_mode = attr.parse(value)?,
                expanded_name!("", "kernelUnitLength") => {
                    let NumberOptionalNumber(NonNegative(x), NonNegative(y)) = attr.parse(value)?;
                    self.kernel_unit_length = Some((x, y))
                }
                expanded_name!("", "preserveAlpha") => self.preserve_alpha = attr.parse(value)?,

                _ => (),
            }
        }

        // Finally, parse the kernel matrix.
        for (attr, value) in attrs
            .iter()
            .filter(|(attr, _)| attr.expanded() == expanded_name!("", "kernelMatrix"))
        {
            self.kernel_matrix = Some({
                let number_of_elements = self.order.0 as usize * self.order.1 as usize;

                // #352: Parse as an unbounded list rather than exact length to prevent aborts due
                //       to huge allocation attempts by underlying Vec::with_capacity().
                let NumberList(v) = NumberList::parse_str(value, NumberListLength::Unbounded)
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

        Ok(())
    }
}

impl ConvolveMatrix {
    pub fn render(
        &self,
        primitive: &ResolvedPrimitive,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        #![allow(clippy::many_single_char_names)]

        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            self.color_interpolation_filters,
        )?;
        let mut bounds = primitive
            .get_bounds(ctx)?
            .add_input(&input_1)
            .into_irect(ctx, draw_ctx);
        let original_bounds = bounds;

        let target_x = match self.target_x {
            Some(x) if x >= self.order.0 => {
                return Err(FilterError::InvalidParameter(
                    "targetX must be less than orderX".to_string(),
                ))
            }
            Some(x) => x,
            None => self.order.0 / 2,
        };

        let target_y = match self.target_y {
            Some(y) if y >= self.order.1 => {
                return Err(FilterError::InvalidParameter(
                    "targetY must be less than orderY".to_string(),
                ))
            }
            Some(y) => y,
            None => self.order.1 / 2,
        };

        let mut input_surface = if self.preserve_alpha {
            // preserve_alpha means we need to premultiply and unpremultiply the values.
            input_1.surface().unpremultiply(bounds)?
        } else {
            input_1.surface().clone()
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

        let divisor = if self.divisor != 0.0 {
            self.divisor
        } else {
            let d = matrix.iter().sum();

            if d != 0.0 {
                d
            } else {
                1.0
            }
        };

        let mut surface = ExclusiveImageSurface::new(
            input_surface.width(),
            input_surface.height(),
            input_1.surface().surface_type(),
        )?;

        surface.modify(&mut |data, stride| {
            for (x, y, pixel) in Pixels::within(&input_surface, bounds) {
                // Compute the convolution rectangle bounds.
                let kernel_bounds = IRect::new(
                    x as i32 - target_x as i32,
                    y as i32 - target_y as i32,
                    x as i32 - target_x as i32 + self.order.0 as i32,
                    y as i32 - target_y as i32 + self.order.1 as i32,
                );

                // Do the convolution.
                let mut r = 0.0;
                let mut g = 0.0;
                let mut b = 0.0;
                let mut a = 0.0;

                for (x, y, pixel) in
                    PixelRectangle::within(&input_surface, bounds, kernel_bounds, self.edge_mode)
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
                    a = a / divisor + self.bias;
                }

                let clamped_a = clamp(a, 0.0, 1.0);

                let compute = |x| {
                    let x = x / divisor + self.bias * a;

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

                data.set_pixel(stride, output_pixel, x, y);
            }
        });

        let mut surface = surface.share()?;

        if let Some((ox, oy)) = scale {
            // Scale the output surface back.
            surface = surface.scale_to(
                ctx.source_graphic().width(),
                ctx.source_graphic().height(),
                original_bounds,
                ox,
                oy,
            )?;

            bounds = original_bounds;
        }

        Ok(FilterResult {
            name: primitive.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }
}

impl FilterEffect for FeConvolveMatrix {
    fn resolve(&self, node: &Node) -> Result<(Primitive, PrimitiveParams), FilterError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        Ok((
            self.base.clone(),
            PrimitiveParams::ConvolveMatrix(ConvolveMatrix {
                in1: self.in1.clone(),
                order: self.order,
                kernel_matrix: self.kernel_matrix.clone(),
                divisor: self.divisor,
                bias: self.bias,
                target_x: self.target_x,
                target_y: self.target_y,
                edge_mode: self.edge_mode,
                kernel_unit_length: self.kernel_unit_length,
                preserve_alpha: self.preserve_alpha,
                color_interpolation_filters: values.color_interpolation_filters(),
            }),
        ))
    }
}

impl Parse for EdgeMode {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "duplicate" => EdgeMode::Duplicate,
            "wrap" => EdgeMode::Wrap,
            "none" => EdgeMode::None,
        )?)
    }
}

// Used for the preserveAlpha attribute
impl Parse for bool {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "false" => false,
            "true" => true,
        )?)
    }
}
