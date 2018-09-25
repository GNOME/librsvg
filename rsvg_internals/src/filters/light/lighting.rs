use std::cell::Cell;
use std::cmp::max;

use cairo::{self, ImageSurface, MatrixTrait};
use cssparser;
use nalgebra::Vector3;
use num_traits::identities::Zero;
use rayon::prelude::*;

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use filters::{
    context::{FilterContext, FilterOutput, FilterResult},
    light::{
        bottom_left_normal,
        bottom_right_normal,
        bottom_row_normal,
        interior_normal,
        left_column_normal,
        light_source::LightSource,
        right_column_normal,
        top_left_normal,
        top_right_normal,
        top_row_normal,
        Normal,
    },
    Filter,
    FilterError,
    PrimitiveWithInput,
};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use state::ColorInterpolationFilters;
use surface_utils::{
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

/// Properties specific to either diffuse or specular lighting.
enum Data {
    Diffuse {
        diffuse_constant: Cell<f64>,
    },
    Specular {
        specular_constant: Cell<f64>,
        specular_exponent: Cell<f64>,
    },
}

/// `Data` but without `Cell`s, needed for sharing between threads.
#[derive(Clone, Copy)]
enum RawData {
    Diffuse {
        diffuse_constant: f64,
    },
    Specular {
        specular_constant: f64,
        specular_exponent: f64,
    },
}

/// The `feDiffuseLighting` and `feSpecularLighting` filter primitives.
pub struct Lighting {
    base: PrimitiveWithInput,
    surface_scale: Cell<f64>,
    kernel_unit_length: Cell<Option<(f64, f64)>>,
    data: Data,
}

impl Lighting {
    /// Constructs a new diffuse `Lighting` with empty properties.
    #[inline]
    pub fn new_diffuse() -> Lighting {
        Lighting {
            data: Data::Diffuse {
                diffuse_constant: Cell::new(1.0),
            },
            ..Self::default()
        }
    }

    /// Constructs a new specular `Lighting` with empty properties.
    #[inline]
    pub fn new_specular() -> Lighting {
        Lighting {
            data: Data::Specular {
                specular_constant: Cell::new(1.0),
                specular_exponent: Cell::new(1.0),
            },
            ..Self::default()
        }
    }
}

impl NodeTrait for Lighting {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::SurfaceScale => self.surface_scale.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
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
                _ => (),
            }
        }

        match self.data {
            Data::Diffuse {
                ref diffuse_constant,
            } => {
                for (_key, attr, value) in pbag.iter() {
                    match attr {
                        Attribute::DiffuseConstant => diffuse_constant.set(
                            parsers::number(value)
                                .map_err(|err| NodeError::attribute_error(attr, err))
                                .and_then(|x| {
                                    if x >= 0.0 {
                                        Ok(x)
                                    } else {
                                        Err(NodeError::value_error(
                                            attr,
                                            "diffuseConstant can't be negative",
                                        ))
                                    }
                                })?,
                        ),
                        _ => (),
                    }
                }
            }
            Data::Specular {
                ref specular_constant,
                ref specular_exponent,
            } => {
                for (_key, attr, value) in pbag.iter() {
                    match attr {
                        Attribute::SpecularConstant => specular_constant.set(
                            parsers::number(value)
                                .map_err(|err| NodeError::attribute_error(attr, err))
                                .and_then(|x| {
                                    if x >= 0.0 {
                                        Ok(x)
                                    } else {
                                        Err(NodeError::value_error(
                                            attr,
                                            "specularConstant can't be negative",
                                        ))
                                    }
                                })?,
                        ),
                        Attribute::SpecularExponent => specular_exponent.set(
                            parsers::number(value)
                                .map_err(|err| NodeError::attribute_error(attr, err))
                                .and_then(|x| {
                                    if x >= 1.0 && x <= 128.0 {
                                        Ok(x)
                                    } else {
                                        Err(NodeError::value_error(
                                            attr,
                                            "specularExponent should be between 1.0 and 128.0",
                                        ))
                                    }
                                })?,
                        ),
                        _ => (),
                    }
                }
            }
        }

        Ok(())
    }
}

impl Filter for Lighting {
    fn render(
        &self,
        node: &RsvgNode,
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

        let scale = self
            .kernel_unit_length
            .get()
            .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

        let surface_scale = self.surface_scale.get();

        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();
        let lighting_color = match values.lighting_color.0 {
            cssparser::Color::CurrentColor => values.color.0,
            cssparser::Color::RGBA(rgba) => rgba,
        };

        let mut light_sources = node
            .children()
            .rev()
            .filter(|c| c.get_type() == NodeType::LightSource);
        let light_source = light_sources.next();
        if light_source.is_none() || light_sources.next().is_some() {
            return Err(FilterError::InvalidLightSourceCount);
        }

        let light_source = light_source.unwrap();
        if light_source.is_in_error() {
            return Err(FilterError::ChildNodeInError);
        }

        let light_source = light_source.get_impl::<LightSource>().unwrap();
        let light_source = light_source.transform(ctx);

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
            let data = self.data.to_raw();

            let compute_output_pixel =
                |mut output_slice: &mut [u8], base_y, x, y, normal: Normal| {
                    let pixel = input_surface.get_pixel(x, y);

                    let scaled_x = f64::from(x) * ox;
                    let scaled_y = f64::from(y) * oy;
                    let z = f64::from(pixel.a) / 255.0 * surface_scale;
                    let light_vector = light_source.vector(scaled_x, scaled_y, z);
                    let light_color = light_source.color(lighting_color, light_vector);

                    let factor = match data {
                        RawData::Diffuse { diffuse_constant } => {
                            let k = if normal.normal.is_zero() {
                                // Common case of (0, 0, 1) normal.
                                light_vector.z
                            } else {
                                let mut n =
                                    normal.normal.map(|x| f64::from(x) * surface_scale / 255.);
                                n.component_mul_assign(&normal.factor);
                                let normal = Vector3::new(n.x, n.y, 1.0);

                                normal.dot(&light_vector) / normal.norm()
                            };

                            diffuse_constant * k
                        }
                        RawData::Specular {
                            specular_constant,
                            specular_exponent,
                        } => {
                            let h = light_vector + Vector3::new(0.0, 0.0, 1.0);
                            let h_norm = h.norm();
                            if h_norm == 0.0 {
                                0.0
                            } else {
                                let k = if normal.normal.is_zero() {
                                    // Common case of (0, 0, 1) normal.
                                    let n_dot_h = h.z / h_norm;
                                    if specular_exponent == 1.0 {
                                        n_dot_h
                                    } else {
                                        n_dot_h.powf(specular_exponent)
                                    }
                                } else {
                                    let mut n =
                                        normal.normal.map(|x| f64::from(x) * surface_scale / 255.);
                                    n.component_mul_assign(&normal.factor);
                                    let normal = Vector3::new(n.x, n.y, 1.0);

                                    let n_dot_h = normal.dot(&h) / normal.norm() / h_norm;
                                    if specular_exponent == 1.0 {
                                        n_dot_h
                                    } else {
                                        n_dot_h.powf(specular_exponent)
                                    }
                                };

                                specular_constant * k
                            }
                        }
                    };

                    let compute = |x| (clamp(factor * f64::from(x), 0.0, 255.0) + 0.5) as u8;

                    let mut output_pixel = Pixel {
                        r: compute(light_color.red),
                        g: compute(light_color.green),
                        b: compute(light_color.blue),
                        a: 255,
                    };

                    if let RawData::Specular { .. } = data {
                        output_pixel.a = max(max(output_pixel.r, output_pixel.g), output_pixel.b);
                    }

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

        let cascaded = node.get_cascaded_values();
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

impl Data {
    #[inline]
    fn to_raw(&self) -> RawData {
        match self {
            Data::Diffuse { diffuse_constant } => RawData::Diffuse {
                diffuse_constant: diffuse_constant.get(),
            },
            Data::Specular {
                specular_constant,
                specular_exponent,
            } => RawData::Specular {
                specular_constant: specular_constant.get(),
                specular_exponent: specular_exponent.get(),
            },
        }
    }
}

impl Default for Lighting {
    #[inline]
    fn default() -> Self {
        Self {
            base: PrimitiveWithInput::new::<Self>(),
            surface_scale: Cell::new(1.0),
            kernel_unit_length: Cell::new(None),

            // The data field is unused in this case.
            data: Data::Diffuse {
                diffuse_constant: Cell::new(1.0),
            },
        }
    }
}
