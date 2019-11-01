use std::cmp::max;

use cairo::{self, ImageSurface};
use cssparser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use nalgebra::Vector3;
use num_traits::identities::Zero;
use rayon::prelude::*;

use crate::drawing_ctx::DrawingCtx;
use crate::error::{AttributeResultExt, NodeError};
use crate::filters::{
    context::{FilterContext, FilterOutput, FilterResult},
    light::{
        bottom_left_normal,
        bottom_right_normal,
        bottom_row_normal,
        interior_normal,
        left_column_normal,
        light_source::FeDistantLight,
        light_source::FePointLight,
        light_source::FeSpotLight,
        light_source::LightSource,
        right_column_normal,
        top_left_normal,
        top_right_normal,
        top_row_normal,
        Normal,
    },
    FilterEffect,
    FilterError,
    PrimitiveWithInput,
};
use crate::node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::parsers;
use crate::property_bag::PropertyBag;
use crate::property_defs::ColorInterpolationFilters;
use crate::surface_utils::{
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt,
    Pixel,
};
use crate::util::clamp;

trait Lighting {
    fn common(&self) -> &Common;

    fn compute_factor(&self, normal: Normal, light_vector: Vector3<f64>) -> f64;
}

struct Common {
    base: PrimitiveWithInput,
    surface_scale: f64,
    kernel_unit_length: Option<(f64, f64)>,
}

impl Common {
    fn new(base: PrimitiveWithInput) -> Self {
        Self {
            base,
            surface_scale: 1.0,
            kernel_unit_length: None,
        }
    }

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "surfaceScale") => {
                    self.surface_scale = parsers::number(value).attribute(attr)?
                }
                expanded_name!(svg "kernelUnitLength") => {
                    self.kernel_unit_length = Some(
                        parsers::number_optional_number(value)
                            .attribute(attr.clone())
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
                    )
                }
                _ => (),
            }
        }

        Ok(())
    }
}

/// The `feDiffuseLighting` filter primitives.
pub struct FeDiffuseLighting {
    common: Common,
    diffuse_constant: f64,
}

impl Default for FeDiffuseLighting {
    fn default() -> Self {
        Self {
            common: Common::new(PrimitiveWithInput::new::<Self>()),
            diffuse_constant: 1.0,
        }
    }
}

impl NodeTrait for FeDiffuseLighting {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.common.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "diffuseConstant") => {
                    self.diffuse_constant = parsers::number(value)
                        .attribute(attr.clone())
                        .and_then(|x| {
                            if x >= 0.0 {
                                Ok(x)
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "diffuseConstant can't be negative",
                                ))
                            }
                        })?;
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Lighting for FeDiffuseLighting {
    #[inline]
    fn common(&self) -> &Common {
        &self.common
    }

    #[inline]
    fn compute_factor(&self, normal: Normal, light_vector: Vector3<f64>) -> f64 {
        let k = if normal.normal.is_zero() {
            // Common case of (0, 0, 1) normal.
            light_vector.z
        } else {
            let mut n = normal
                .normal
                .map(|x| f64::from(x) * self.common().surface_scale / 255.);
            n.component_mul_assign(&normal.factor);
            let normal = Vector3::new(n.x, n.y, 1.0);

            normal.dot(&light_vector) / normal.norm()
        };

        self.diffuse_constant * k
    }
}

/// The `feSpecularLighting` filter primitives.
pub struct FeSpecularLighting {
    common: Common,
    specular_constant: f64,
    specular_exponent: f64,
}

impl Default for FeSpecularLighting {
    fn default() -> Self {
        Self {
            common: Common::new(PrimitiveWithInput::new::<Self>()),
            specular_constant: 1.0,
            specular_exponent: 1.0,
        }
    }
}

impl NodeTrait for FeSpecularLighting {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.common.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "specularConstant") => {
                    self.specular_constant = parsers::number(value)
                        .attribute(attr.clone())
                        .and_then(|x| {
                            if x >= 0.0 {
                                Ok(x)
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "specularConstant can't be negative",
                                ))
                            }
                        })?;
                }
                expanded_name!(svg "specularExponent") => {
                    self.specular_exponent = parsers::number(value)
                        .attribute(attr.clone())
                        .and_then(|x| {
                            if x >= 1.0 && x <= 128.0 {
                                Ok(x)
                            } else {
                                Err(NodeError::value_error(
                                    attr,
                                    "specularExponent should be between 1.0 and 128.0",
                                ))
                            }
                        })?;
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Lighting for FeSpecularLighting {
    #[inline]
    fn common(&self) -> &Common {
        &self.common
    }

    #[inline]
    fn compute_factor(&self, normal: Normal, light_vector: Vector3<f64>) -> f64 {
        let h = light_vector + Vector3::new(0.0, 0.0, 1.0);
        let h_norm = h.norm();

        if h_norm == 0.0 {
            return 0.0;
        }

        let k = if normal.normal.is_zero() {
            // Common case of (0, 0, 1) normal.
            let n_dot_h = h.z / h_norm;
            if self.specular_exponent == 1.0 {
                n_dot_h
            } else {
                n_dot_h.powf(self.specular_exponent)
            }
        } else {
            let mut n = normal
                .normal
                .map(|x| f64::from(x) * self.common().surface_scale / 255.);
            n.component_mul_assign(&normal.factor);
            let normal = Vector3::new(n.x, n.y, 1.0);

            let n_dot_h = normal.dot(&h) / normal.norm() / h_norm;
            if self.specular_exponent == 1.0 {
                n_dot_h
            } else {
                n_dot_h.powf(self.specular_exponent)
            }
        };

        self.specular_constant * k
    }
}

// We cannot use a blanket impl<T: Lighting> Filter for T because we do
// not want to make the Lighting trait public, so we use a macro
macro_rules! impl_lighting_filter {
    ($lighting_type:ty, $alpha_func:ident) => {
        impl FilterEffect for $lighting_type {
            fn render(
                &self,
                node: &RsvgNode,
                ctx: &FilterContext,
                draw_ctx: &mut DrawingCtx,
            ) -> Result<FilterResult, FilterError> {
                let input = self.common().base.get_input(ctx, draw_ctx)?;
                let mut bounds = self
                    .common()
                    .base
                    .get_bounds(ctx)
                    .add_input(&input)
                    .into_irect(draw_ctx);
                let original_bounds = bounds;

                let scale = self
                    .common()
                    .kernel_unit_length
                    .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

                let cascaded = CascadedValues::new_from_node(node);
                let values = cascaded.get();
                let lighting_color = match values.lighting_color.0 {
                    cssparser::Color::CurrentColor => values.color.0,
                    cssparser::Color::RGBA(rgba) => rgba,
                };

                let light_source = find_light_source(node, ctx)?;
                let mut input_surface = input.surface().clone();

                if let Some((ox, oy)) = scale {
                    // Scale the input surface to match kernel_unit_length.
                    let (new_surface, new_bounds) = input_surface.scale(bounds, 1.0 / ox, 1.0 / oy)?;

                    input_surface = new_surface;
                    bounds = new_bounds;
                }

                // Check if the surface is too small for normal computation. This case is unspecified;
                // WebKit doesn't render anything in this case.
                if bounds.x1 < bounds.x0 + 2 || bounds.y1 < bounds.y0 + 2 {
                    return Err(FilterError::LightingInputTooSmall);
                }

                let (ox, oy) = scale.unwrap_or((1.0, 1.0));

                let mut output_surface = ImageSurface::create(
                    cairo::Format::ARgb32,
                    input_surface.width(),
                    input_surface.height(),
                )?;

                let output_stride = output_surface.get_stride() as usize;
                {
                    let mut output_data = output_surface.get_data().unwrap();
                    let output_slice = &mut *output_data;

                    let compute_output_pixel =
                        |mut output_slice: &mut [u8], base_y, x, y, normal: Normal| {
                            let pixel = input_surface.get_pixel(x, y);

                            let scaled_x = f64::from(x) * ox;
                            let scaled_y = f64::from(y) * oy;
                            let z = f64::from(pixel.a) / 255.0 * self.common().surface_scale;
                            let light_vector = light_source.vector(scaled_x, scaled_y, z);
                            let light_color = light_source.color(lighting_color, light_vector);

                            // compute the factor just once for the three colors
                            let factor = self.compute_factor(normal, light_vector);
                            let compute = |x| (clamp(factor * f64::from(x), 0.0, 255.0) + 0.5) as u8;

                            let r = compute(light_color.red);
                            let g = compute(light_color.green);
                            let b = compute(light_color.blue);
                            let a = $alpha_func(r, g, b);

                            let output_pixel = Pixel {
                                r,
                                g,
                                b,
                                a,
                            };

                            output_slice.set_pixel(output_stride, output_pixel, x, y - base_y);
                        };

                    // Top left.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x0 as u32,
                        bounds.y0 as u32,
                        top_left_normal(&input_surface, bounds),
                    );

                    // Top right.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x1 as u32 - 1,
                        bounds.y0 as u32,
                        top_right_normal(&input_surface, bounds),
                    );

                    // Bottom left.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x0 as u32,
                        bounds.y1 as u32 - 1,
                        bottom_left_normal(&input_surface, bounds),
                    );

                    // Bottom right.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x1 as u32 - 1,
                        bounds.y1 as u32 - 1,
                        bottom_right_normal(&input_surface, bounds),
                    );

                    if bounds.x1 - bounds.x0 >= 3 {
                        // Top row.
                        for x in bounds.x0 as u32 + 1..bounds.x1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                x,
                                bounds.y0 as u32,
                                top_row_normal(&input_surface, bounds, x),
                            );
                        }

                        // Bottom row.
                        for x in bounds.x0 as u32 + 1..bounds.x1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                x,
                                bounds.y1 as u32 - 1,
                                bottom_row_normal(&input_surface, bounds, x),
                            );
                        }
                    }

                    if bounds.y1 - bounds.y0 >= 3 {
                        // Left column.
                        for y in bounds.y0 as u32 + 1..bounds.y1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                bounds.x0 as u32,
                                y,
                                left_column_normal(&input_surface, bounds, y),
                            );
                        }

                        // Right column.
                        for y in bounds.y0 as u32 + 1..bounds.y1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                bounds.x1 as u32 - 1,
                                y,
                                right_column_normal(&input_surface, bounds, y),
                            );
                        }
                    }

                    if bounds.x1 - bounds.x0 >= 3 && bounds.y1 - bounds.y0 >= 3 {
                        // Interior pixels.
                        let first_row = bounds.y0 as u32 + 1;
                        let one_past_last_row = bounds.y1 as u32 - 1;
                        let first_pixel = (first_row as usize) * output_stride;
                        let one_past_last_pixel = (one_past_last_row as usize) * output_stride;

                        output_slice[first_pixel..one_past_last_pixel]
                            .par_chunks_mut(output_stride)
                            .zip(first_row..one_past_last_row)
                            .for_each(|(slice, y)| {
                                for x in bounds.x0 as u32 + 1..bounds.x1 as u32 - 1 {
                                    compute_output_pixel(
                                        slice,
                                        y,
                                        x,
                                        y,
                                        interior_normal(&input_surface, bounds, x, y),
                                    );
                                }
                            });
                    }
                }

                let cascaded = CascadedValues::new_from_node(node);
                let values = cascaded.get();
                // The generated color values are in the color space determined by
                // color-interpolation-filters.
                let surface_type =
                    if values.color_interpolation_filters == ColorInterpolationFilters::LinearRgb {
                        SurfaceType::LinearRgb
                    } else {
                        SurfaceType::SRgb
                    };
                let mut output_surface = SharedImageSurface::new(output_surface, surface_type)?;

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
                    name: self.common().base.result.clone(),
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
    }
}

const fn diffuse_alpha(_r: u8, _g: u8, _b: u8) -> u8 {
    255
}

fn specular_alpha(r: u8, g: u8, b: u8) -> u8 {
    max(max(r, g), b)
}

impl_lighting_filter!(FeDiffuseLighting, diffuse_alpha);
impl_lighting_filter!(FeSpecularLighting, specular_alpha);

fn find_light_source(node: &RsvgNode, ctx: &FilterContext) -> Result<LightSource, FilterError> {
    let mut light_sources = node
        .children()
        .rev()
        .filter(|c| match c.borrow().get_type() {
            NodeType::FeDistantLight | NodeType::FePointLight | NodeType::FeSpotLight => true,
            _ => false,
        });

    let node = light_sources.next();
    if node.is_none() || light_sources.next().is_some() {
        return Err(FilterError::InvalidLightSourceCount);
    }

    let node = node.unwrap();
    if node.borrow().is_in_error() {
        return Err(FilterError::ChildNodeInError);
    }

    let light_source = match node.borrow().get_type() {
        NodeType::FeDistantLight => node.borrow().get_impl::<FeDistantLight>().transform(ctx),
        NodeType::FePointLight => node.borrow().get_impl::<FePointLight>().transform(ctx),
        NodeType::FeSpotLight => node.borrow().get_impl::<FeSpotLight>().transform(ctx),
        _ => unreachable!(),
    };

    Ok(light_source)
}
