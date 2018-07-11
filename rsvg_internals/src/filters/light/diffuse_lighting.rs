use std::cell::Cell;

use cairo::{self, ImageSurface};
use cssparser;

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use filters::{
    context::{FilterContext, FilterOutput, FilterResult},
    light::{light_source::LightSource, normal},
    Filter,
    FilterError,
    PrimitiveWithInput,
};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

/// The `feDiffuseLighting` filter primitive.
pub struct DiffuseLighting {
    base: PrimitiveWithInput,
    surface_scale: Cell<f64>,
    diffuse_constant: Cell<f64>,
    kernel_unit_length: Cell<Option<(f64, f64)>>,
}

impl DiffuseLighting {
    /// Constructs a new `DiffuseLighting` with empty properties.
    #[inline]
    pub fn new() -> DiffuseLighting {
        DiffuseLighting {
            base: PrimitiveWithInput::new::<Self>(),
            surface_scale: Cell::new(1.0),
            diffuse_constant: Cell::new(1.0),
            kernel_unit_length: Cell::new(None),
        }
    }
}

impl NodeTrait for DiffuseLighting {
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
                Attribute::DiffuseConstant => self.diffuse_constant.set(
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

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
    }
}

impl Filter for DiffuseLighting {
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

        let scale = self.kernel_unit_length.get().map(|(dx, dy)| {
            let paffine = ctx.paffine();
            let ox = paffine.xx * dx + paffine.xy * dy;
            let oy = paffine.yx * dx + paffine.yy * dy;

            (ox, oy)
        });

        let surface_scale = self.surface_scale.get();
        let diffuse_constant = self.diffuse_constant.get();

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
            let (new_surface, new_bounds) = input_surface
                .scale(bounds, 1.0 / ox, 1.0 / oy)
                .map_err(FilterError::IntermediateSurfaceCreation)?;

            input_surface = new_surface;
            bounds = new_bounds;
        }

        let mut output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            input_surface.width(),
            input_surface.height(),
        ).map_err(FilterError::IntermediateSurfaceCreation)?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, _pixel) in Pixels::new(&input_surface, bounds) {
                let normal = normal(&input_surface, bounds, x, y, surface_scale);
                let light_vector = light_source.vector(&input_surface, x, y, surface_scale, ctx);
                let light_color = light_source.color(lighting_color, &light_vector, ctx);

                let n_dot_l = normal.dot(&light_vector);
                let compute =
                    |x| clamp(diffuse_constant * n_dot_l * f64::from(x), 0.0, 255.0).round() as u8;

                let output_pixel = Pixel {
                    r: compute(light_color.red),
                    g: compute(light_color.green),
                    b: compute(light_color.blue),
                    a: 255,
                }.premultiply();
                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        let mut output_surface = SharedImageSurface::new(output_surface)
            .map_err(FilterError::BadIntermediateSurfaceStatus)?;

        if let Some((ox, oy)) = scale {
            // Scale the output surface back.
            output_surface = output_surface
                .scale_to(
                    ctx.source_graphic().width(),
                    ctx.source_graphic().height(),
                    original_bounds,
                    ox,
                    oy,
                )
                .map_err(FilterError::IntermediateSurfaceCreation)?;
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
    fn is_affected_by_color_interpolation_filters() -> bool {
        true
    }
}
