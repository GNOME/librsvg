use std::cell::Cell;
use std::cmp::max;

use cairo::{self, ImageSurface, MatrixTrait};
use cssparser;

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use filters::{
    context::{FilterContext, FilterOutput, FilterResult},
    light::{light_source::LightSource, normal, normalize},
    Filter,
    FilterError,
    PrimitiveWithInput,
};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

/// The `feSpecularLighting` filter primitive.
pub struct SpecularLighting {
    base: PrimitiveWithInput,
    surface_scale: Cell<f64>,
    specular_constant: Cell<f64>,
    specular_exponent: Cell<f64>,
    kernel_unit_length: Cell<Option<(f64, f64)>>,
}

impl SpecularLighting {
    /// Constructs a new `SpecularLighting` with empty properties.
    #[inline]
    pub fn new() -> SpecularLighting {
        SpecularLighting {
            base: PrimitiveWithInput::new::<Self>(),
            surface_scale: Cell::new(1.0),
            specular_constant: Cell::new(1.0),
            specular_exponent: Cell::new(1.0),
            kernel_unit_length: Cell::new(None),
        }
    }
}

impl NodeTrait for SpecularLighting {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::SurfaceScale => self
                    .surface_scale
                    .set(parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?),
                Attribute::SpecularConstant => self.specular_constant.set(
                    parsers::number(value)
                        .map_err(|err| NodeError::parse_error(attr, err))
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
                Attribute::SpecularExponent => self.specular_exponent.set(
                    parsers::number(value)
                        .map_err(|err| NodeError::parse_error(attr, err))
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
                Attribute::KernelUnitLength => self.kernel_unit_length.set(Some(
                    parsers::number_optional_number(value)
                        .map_err(|err| NodeError::parse_error(attr, err))
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

        Ok(())
    }
}

impl Filter for SpecularLighting {
    fn render(
        &self,
        node: &RsvgNode,
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

        let scale = self
            .kernel_unit_length
            .get()
            .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

        let surface_scale = self.surface_scale.get();
        let specular_constant = self.specular_constant.get();
        let specular_exponent = self.specular_exponent.get();

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
        let light_source = light_source.get_impl::<LightSource>().unwrap();

        let mut input_surface = input.surface().clone();

        if let Some((ox, oy)) = scale {
            // Scale the input surface to match kernel_unit_length.
            let (new_surface, new_bounds) = input_surface.scale(bounds, 1.0 / ox, 1.0 / oy)?;

            let new_surface = SharedImageSurface::new(new_surface)?;

            input_surface = new_surface;
            bounds = new_bounds;
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

            for (x, y, pixel) in Pixels::new(&input_surface, bounds) {
                let normal = normal(&input_surface, bounds, x, y, surface_scale);

                let scaled_x = f64::from(x) / ox;
                let scaled_y = f64::from(y) / oy;
                let z = f64::from(pixel.a) / 255.0 * surface_scale;
                let light_vector = light_source.vector(scaled_x, scaled_y, z, ctx);

                let light_color = light_source.color(lighting_color, &light_vector, ctx);

                let mut h = light_vector + vector![0.0, 0.0, 1.0];
                let _ = normalize(&mut h);

                let n_dot_h = normal.dot(&h);
                let factor = specular_constant * n_dot_h.powf(specular_exponent);
                let compute = |x| clamp(factor * f64::from(x), 0.0, 255.0).round() as u8;

                let mut output_pixel = Pixel {
                    r: compute(light_color.red),
                    g: compute(light_color.green),
                    b: compute(light_color.blue),
                    a: 0,
                };
                output_pixel.a = max(max(output_pixel.r, output_pixel.g), output_pixel.b);
                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        let mut output_surface = SharedImageSurface::new(output_surface)?;

        if let Some((ox, oy)) = scale {
            // Scale the output surface back.
            output_surface = SharedImageSurface::new(output_surface.scale_to(
                ctx.source_graphic().width(),
                ctx.source_graphic().height(),
                original_bounds,
                ox,
                oy,
            )?)?;

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
