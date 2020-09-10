//! Lighting filters and light nodes.

use float_cmp::approx_eq;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use matches::matches;
use nalgebra::{Vector2, Vector3};
use num_traits::identities::Zero;
use rayon::prelude::*;
use std::cmp::max;

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::*;
use crate::filters::{
    context::{FilterContext, FilterOutput, FilterResult},
    FilterEffect, FilterError, PrimitiveWithInput,
};
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::parsers::{NumberOptionalNumber, ParseValue};
use crate::rect::IRect;
use crate::surface_utils::{
    shared_surface::{ExclusiveImageSurface, SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;

/// A light source with affine transformations applied.
pub enum LightSource {
    Distant {
        azimuth: f64,
        elevation: f64,
    },
    Point {
        origin: Vector3<f64>,
    },
    Spot {
        origin: Vector3<f64>,
        direction: Vector3<f64>,
        specular_exponent: f64,
        limiting_cone_angle: Option<f64>,
    },
}

impl LightSource {
    /// Returns the unit (or null) vector from the image sample to the light.
    #[inline]
    pub fn vector(&self, x: f64, y: f64, z: f64) -> Vector3<f64> {
        match self {
            LightSource::Distant { azimuth, elevation } => {
                let azimuth = azimuth.to_radians();
                let elevation = elevation.to_radians();
                Vector3::new(
                    azimuth.cos() * elevation.cos(),
                    azimuth.sin() * elevation.cos(),
                    elevation.sin(),
                )
            }
            LightSource::Point { origin } | LightSource::Spot { origin, .. } => {
                let mut v = origin - Vector3::new(x, y, z);
                let _ = v.try_normalize_mut(0.0);
                v
            }
        }
    }

    /// Returns the color of the light.
    #[inline]
    pub fn color(
        &self,
        lighting_color: cssparser::RGBA,
        light_vector: Vector3<f64>,
    ) -> cssparser::RGBA {
        match self {
            LightSource::Spot {
                direction,
                specular_exponent,
                limiting_cone_angle,
                ..
            } => {
                let minus_l_dot_s = -light_vector.dot(&direction);
                if minus_l_dot_s <= 0.0 {
                    return cssparser::RGBA::transparent();
                }

                if let Some(limiting_cone_angle) = limiting_cone_angle {
                    if minus_l_dot_s < limiting_cone_angle.to_radians().cos() {
                        return cssparser::RGBA::transparent();
                    }
                }

                let factor = minus_l_dot_s.powf(*specular_exponent);
                let compute = |x| (clamp(f64::from(x) * factor, 0.0, 255.0) + 0.5) as u8;

                cssparser::RGBA {
                    red: compute(lighting_color.red),
                    green: compute(lighting_color.green),
                    blue: compute(lighting_color.blue),
                    alpha: 255,
                }
            }
            _ => lighting_color,
        }
    }
}

#[derive(Default)]
pub struct FeDistantLight {
    azimuth: f64,
    elevation: f64,
}

impl FeDistantLight {
    pub fn transform(&self, _ctx: &FilterContext) -> LightSource {
        LightSource::Distant {
            azimuth: self.azimuth,
            elevation: self.elevation,
        }
    }
}

impl SetAttributes for FeDistantLight {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "azimuth") => self.azimuth = attr.parse(value)?,
                expanded_name!("", "elevation") => self.elevation = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for FeDistantLight {}

#[derive(Default)]
pub struct FePointLight {
    x: f64,
    y: f64,
    z: f64,
}

impl FePointLight {
    pub fn transform(&self, ctx: &FilterContext) -> LightSource {
        let (x, y) = ctx.paffine().transform_point(self.x, self.y);
        let z = ctx.transform_dist(self.z);

        LightSource::Point {
            origin: Vector3::new(x, y, z),
        }
    }
}

impl SetAttributes for FePointLight {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "z") => self.z = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for FePointLight {}

#[derive(Default)]
pub struct FeSpotLight {
    x: f64,
    y: f64,
    z: f64,
    points_at_x: f64,
    points_at_y: f64,
    points_at_z: f64,
    specular_exponent: f64,
    limiting_cone_angle: Option<f64>,
}

impl FeSpotLight {
    pub fn transform(&self, ctx: &FilterContext) -> LightSource {
        let (x, y) = ctx.paffine().transform_point(self.x, self.y);
        let z = ctx.transform_dist(self.z);
        let (points_at_x, points_at_y) = ctx
            .paffine()
            .transform_point(self.points_at_x, self.points_at_y);
        let points_at_z = ctx.transform_dist(self.points_at_z);

        let origin = Vector3::new(x, y, z);
        let mut direction = Vector3::new(points_at_x, points_at_y, points_at_z) - origin;
        let _ = direction.try_normalize_mut(0.0);

        LightSource::Spot {
            origin,
            direction,
            specular_exponent: self.specular_exponent,
            limiting_cone_angle: self.limiting_cone_angle,
        }
    }
}

impl SetAttributes for FeSpotLight {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "z") => self.z = attr.parse(value)?,
                expanded_name!("", "pointsAtX") => self.points_at_x = attr.parse(value)?,
                expanded_name!("", "pointsAtY") => self.points_at_y = attr.parse(value)?,
                expanded_name!("", "pointsAtZ") => self.points_at_z = attr.parse(value)?,

                expanded_name!("", "specularExponent") => {
                    self.specular_exponent = attr.parse(value)?
                }

                expanded_name!("", "limitingConeAngle") => {
                    self.limiting_cone_angle = Some(attr.parse(value)?)
                }

                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for FeSpotLight {}

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
}

impl SetAttributes for Common {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "surfaceScale") => self.surface_scale = attr.parse(value)?,

                expanded_name!("", "kernelUnitLength") => {
                    let NumberOptionalNumber(x, y) =
                        attr.parse_and_validate(value, |v: NumberOptionalNumber<f64>| {
                            if v.0 > 0.0 && v.1 > 0.0 {
                                Ok(v)
                            } else {
                                Err(ValueErrorKind::value_error(
                                    "kernelUnitLength can't be less or equal to zero",
                                ))
                            }
                        })?;

                    self.kernel_unit_length = Some((x, y));
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

impl SetAttributes for FeDiffuseLighting {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.common.set_attributes(attrs)?;
        let result = attrs
            .iter()
            .find(|(attr, _)| attr.expanded() == expanded_name!("", "diffuseConstant"))
            .and_then(|(attr, value)| {
                attr.parse_and_validate(value, |x| {
                    if x >= 0.0 {
                        Ok(x)
                    } else {
                        Err(ValueErrorKind::value_error(
                            "diffuseConstant can't be negative",
                        ))
                    }
                })
                .ok()
            });
        if let Some(diffuse_constant) = result {
            self.diffuse_constant = diffuse_constant;
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

impl SetAttributes for FeSpecularLighting {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.common.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "specularConstant") => {
                    self.specular_constant = attr.parse_and_validate(value, |x| {
                        if x >= 0.0 {
                            Ok(x)
                        } else {
                            Err(ValueErrorKind::value_error(
                                "specularConstant can't be negative",
                            ))
                        }
                    })?;
                }
                expanded_name!("", "specularExponent") => {
                    self.specular_exponent = attr.parse_and_validate(value, |x| {
                        if x >= 1.0 && x <= 128.0 {
                            Ok(x)
                        } else {
                            Err(ValueErrorKind::value_error(
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
            if approx_eq!(f64, self.specular_exponent, 1.0) {
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
            if approx_eq!(f64, self.specular_exponent, 1.0) {
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
                node: &Node,
                ctx: &FilterContext,
                acquired_nodes: &mut AcquiredNodes,
                draw_ctx: &mut DrawingCtx,
            ) -> Result<FilterResult, FilterError> {
                let input = self
                    .common()
                    .base
                    .get_input(ctx, acquired_nodes, draw_ctx)?;
                let mut bounds = self
                    .common()
                    .base
                    .get_bounds(ctx, node.parent().as_ref())?
                    .add_input(&input)
                    .into_irect(draw_ctx);
                let original_bounds = bounds;

                let scale = self
                    .common()
                    .kernel_unit_length
                    .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

                let cascaded = CascadedValues::new_from_node(node);
                let values = cascaded.get();
                let lighting_color = match values.lighting_color().0 {
                    cssparser::Color::CurrentColor => values.color().0,
                    cssparser::Color::RGBA(rgba) => rgba,
                };

                let light_source = find_light_source(node, ctx)?;
                let mut input_surface = input.surface().clone();

                if let Some((ox, oy)) = scale {
                    // Scale the input surface to match kernel_unit_length.
                    let (new_surface, new_bounds) =
                        input_surface.scale(bounds, 1.0 / ox, 1.0 / oy)?;

                    input_surface = new_surface;
                    bounds = new_bounds;
                }

                let (bounds_w, bounds_h) = bounds.size();

                // Check if the surface is too small for normal computation. This case is
                // unspecified; WebKit doesn't render anything in this case.
                if bounds_w < 2 || bounds_h < 2 {
                    return Err(FilterError::LightingInputTooSmall);
                }

                let (ox, oy) = scale.unwrap_or((1.0, 1.0));

                let cascaded = CascadedValues::new_from_node(node);
                let values = cascaded.get();
                // The generated color values are in the color space determined by
                // color-interpolation-filters.
                let surface_type = SurfaceType::from(values.color_interpolation_filters());

                let mut surface = ExclusiveImageSurface::new(
                    input_surface.width(),
                    input_surface.height(),
                    surface_type,
                )?;

                {
                    let output_stride = surface.stride() as usize;
                    let mut output_data = surface.get_data();
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
                            let compute =
                                |x| (clamp(factor * f64::from(x), 0.0, 255.0) + 0.5) as u8;

                            let r = compute(light_color.red);
                            let g = compute(light_color.green);
                            let b = compute(light_color.blue);
                            let a = $alpha_func(r, g, b);

                            let output_pixel = Pixel { r, g, b, a };

                            output_slice.set_pixel(output_stride, output_pixel, x, y - base_y);
                        };

                    // Top left.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x0 as u32,
                        bounds.y0 as u32,
                        Normal::top_left(&input_surface, bounds),
                    );

                    // Top right.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x1 as u32 - 1,
                        bounds.y0 as u32,
                        Normal::top_right(&input_surface, bounds),
                    );

                    // Bottom left.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x0 as u32,
                        bounds.y1 as u32 - 1,
                        Normal::bottom_left(&input_surface, bounds),
                    );

                    // Bottom right.
                    compute_output_pixel(
                        output_slice,
                        0,
                        bounds.x1 as u32 - 1,
                        bounds.y1 as u32 - 1,
                        Normal::bottom_right(&input_surface, bounds),
                    );

                    if bounds_w >= 3 {
                        // Top row.
                        for x in bounds.x0 as u32 + 1..bounds.x1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                x,
                                bounds.y0 as u32,
                                Normal::top_row(&input_surface, bounds, x),
                            );
                        }

                        // Bottom row.
                        for x in bounds.x0 as u32 + 1..bounds.x1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                x,
                                bounds.y1 as u32 - 1,
                                Normal::bottom_row(&input_surface, bounds, x),
                            );
                        }
                    }

                    if bounds_h >= 3 {
                        // Left column.
                        for y in bounds.y0 as u32 + 1..bounds.y1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                bounds.x0 as u32,
                                y,
                                Normal::left_column(&input_surface, bounds, y),
                            );
                        }

                        // Right column.
                        for y in bounds.y0 as u32 + 1..bounds.y1 as u32 - 1 {
                            compute_output_pixel(
                                output_slice,
                                0,
                                bounds.x1 as u32 - 1,
                                y,
                                Normal::right_column(&input_surface, bounds, y),
                            );
                        }
                    }

                    if bounds_w >= 3 && bounds_h >= 3 {
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
                                        Normal::interior(&input_surface, bounds, x, y),
                                    );
                                }
                            });
                    }
                }

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
                    name: self.common().base.result.clone(),
                    output: FilterOutput { surface, bounds },
                })
            }

            #[inline]
            fn is_affected_by_color_interpolation_filters(&self) -> bool {
                true
            }
        }
    };
}

const fn diffuse_alpha(_r: u8, _g: u8, _b: u8) -> u8 {
    255
}

fn specular_alpha(r: u8, g: u8, b: u8) -> u8 {
    max(max(r, g), b)
}

impl_lighting_filter!(FeDiffuseLighting, diffuse_alpha);
impl_lighting_filter!(FeSpecularLighting, specular_alpha);

fn find_light_source(node: &Node, ctx: &FilterContext) -> Result<LightSource, FilterError> {
    let mut light_sources = node.children().rev().filter(|c| {
        c.is_element() && matches!(*c.borrow_element(), Element::FeDistantLight(_) | Element::FePointLight(_) | Element::FeSpotLight(_))
    });

    let node = light_sources.next();
    if node.is_none() || light_sources.next().is_some() {
        return Err(FilterError::InvalidLightSourceCount);
    }

    let node = node.unwrap();
    let elt = node.borrow_element();

    if elt.is_in_error() {
        return Err(FilterError::ChildNodeInError);
    }

    let light_source = match *elt {
        Element::FeDistantLight(ref l) => l.transform(ctx),
        Element::FePointLight(ref l) => l.transform(ctx),
        Element::FeSpotLight(ref l) => l.transform(ctx),
        _ => unreachable!(),
    };

    Ok(light_source)
}

/// 2D normal and factor stored separately.
///
/// The normal needs to be multiplied by `surface_scale * factor / 255` and
/// normalized with 1 as the z component.
/// pub for the purpose of accessing this from benchmarks.
#[derive(Debug, Clone, Copy)]
pub struct Normal {
    pub factor: Vector2<f64>,
    pub normal: Vector2<i16>,
}

impl Normal {
    #[inline]
    fn new(factor_x: f64, nx: i16, factor_y: f64, ny: i16) -> Normal {
        // Negative nx and ny to account for the different coordinate system.
        Normal {
            factor: Vector2::new(factor_x, factor_y),
            normal: Vector2::new(-nx, -ny),
        }
    }

    /// Computes and returns the normal vector for the top left pixel for light filters.
    #[inline]
    pub fn top_left(surface: &SharedImageSurface, bounds: IRect) -> Normal {
        // Surface needs to be at least 2×2.
        assert!(bounds.width() >= 2);
        assert!(bounds.height() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let (x, y) = (bounds.x0 as u32, bounds.y0 as u32);

        let center = get(x, y);
        let right = get(x + 1, y);
        let bottom = get(x, y + 1);
        let bottom_right = get(x + 1, y + 1);

        Self::new(
            2. / 3.,
            -2 * center + 2 * right - bottom + bottom_right,
            2. / 3.,
            -2 * center - right + 2 * bottom + bottom_right,
        )
    }

    /// Computes and returns the normal vector for the top row pixels for light filters.
    #[inline]
    pub fn top_row(surface: &SharedImageSurface, bounds: IRect, x: u32) -> Normal {
        assert!(x as i32 > bounds.x0);
        assert!((x as i32) + 1 < bounds.x1);
        assert!(bounds.height() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let y = bounds.y0 as u32;

        let left = get(x - 1, y);
        let center = get(x, y);
        let right = get(x + 1, y);
        let bottom_left = get(x - 1, y + 1);
        let bottom = get(x, y + 1);
        let bottom_right = get(x + 1, y + 1);

        Self::new(
            1. / 3.,
            -2 * left + 2 * right - bottom_left + bottom_right,
            1. / 2.,
            -left - 2 * center - right + bottom_left + 2 * bottom + bottom_right,
        )
    }

    /// Computes and returns the normal vector for the top right pixel for light filters.
    #[inline]
    pub fn top_right(surface: &SharedImageSurface, bounds: IRect) -> Normal {
        // Surface needs to be at least 2×2.
        assert!(bounds.width() >= 2);
        assert!(bounds.height() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let (x, y) = (bounds.x1 as u32 - 1, bounds.y0 as u32);

        let left = get(x - 1, y);
        let center = get(x, y);
        let bottom_left = get(x - 1, y + 1);
        let bottom = get(x, y + 1);

        Self::new(
            2. / 3.,
            -2 * left + 2 * center - bottom_left + bottom,
            2. / 3.,
            -left - 2 * center + bottom_left + 2 * bottom,
        )
    }

    /// Computes and returns the normal vector for the left column pixels for light filters.
    #[inline]
    pub fn left_column(surface: &SharedImageSurface, bounds: IRect, y: u32) -> Normal {
        assert!(y as i32 > bounds.y0);
        assert!((y as i32) + 1 < bounds.y1);
        assert!(bounds.width() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let x = bounds.x0 as u32;

        let top = get(x, y - 1);
        let top_right = get(x + 1, y - 1);
        let center = get(x, y);
        let right = get(x + 1, y);
        let bottom = get(x, y + 1);
        let bottom_right = get(x + 1, y + 1);

        Self::new(
            1. / 2.,
            -top + top_right - 2 * center + 2 * right - bottom + bottom_right,
            1. / 3.,
            -2 * top - top_right + 2 * bottom + bottom_right,
        )
    }

    /// Computes and returns the normal vector for the interior pixels for light filters.
    #[inline]
    pub fn interior(surface: &SharedImageSurface, bounds: IRect, x: u32, y: u32) -> Normal {
        assert!(x as i32 > bounds.x0);
        assert!((x as i32) + 1 < bounds.x1);
        assert!(y as i32 > bounds.y0);
        assert!((y as i32) + 1 < bounds.y1);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);

        let top_left = get(x - 1, y - 1);
        let top = get(x, y - 1);
        let top_right = get(x + 1, y - 1);
        let left = get(x - 1, y);
        let right = get(x + 1, y);
        let bottom_left = get(x - 1, y + 1);
        let bottom = get(x, y + 1);
        let bottom_right = get(x + 1, y + 1);

        Self::new(
            1. / 4.,
            -top_left + top_right - 2 * left + 2 * right - bottom_left + bottom_right,
            1. / 4.,
            -top_left - 2 * top - top_right + bottom_left + 2 * bottom + bottom_right,
        )
    }

    /// Computes and returns the normal vector for the right column pixels for light filters.
    #[inline]
    pub fn right_column(surface: &SharedImageSurface, bounds: IRect, y: u32) -> Normal {
        assert!(y as i32 > bounds.y0);
        assert!((y as i32) + 1 < bounds.y1);
        assert!(bounds.width() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let x = bounds.x1 as u32 - 1;

        let top_left = get(x - 1, y - 1);
        let top = get(x, y - 1);
        let left = get(x - 1, y);
        let center = get(x, y);
        let bottom_left = get(x - 1, y + 1);
        let bottom = get(x, y + 1);

        Self::new(
            1. / 2.,
            -top_left + top - 2 * left + 2 * center - bottom_left + bottom,
            1. / 3.,
            -top_left - 2 * top + bottom_left + 2 * bottom,
        )
    }

    /// Computes and returns the normal vector for the bottom left pixel for light filters.
    #[inline]
    pub fn bottom_left(surface: &SharedImageSurface, bounds: IRect) -> Normal {
        // Surface needs to be at least 2×2.
        assert!(bounds.width() >= 2);
        assert!(bounds.height() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let (x, y) = (bounds.x0 as u32, bounds.y1 as u32 - 1);

        let top = get(x, y - 1);
        let top_right = get(x + 1, y - 1);
        let center = get(x, y);
        let right = get(x + 1, y);

        Self::new(
            2. / 3.,
            -top + top_right - 2 * center + 2 * right,
            2. / 3.,
            -2 * top - top_right + 2 * center + right,
        )
    }

    /// Computes and returns the normal vector for the bottom row pixels for light filters.
    #[inline]
    pub fn bottom_row(surface: &SharedImageSurface, bounds: IRect, x: u32) -> Normal {
        assert!(x as i32 > bounds.x0);
        assert!((x as i32) + 1 < bounds.x1);
        assert!(bounds.height() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let y = bounds.y1 as u32 - 1;

        let top_left = get(x - 1, y - 1);
        let top = get(x, y - 1);
        let top_right = get(x + 1, y - 1);
        let left = get(x - 1, y);
        let center = get(x, y);
        let right = get(x + 1, y);

        Self::new(
            1. / 3.,
            -top_left + top_right - 2 * left + 2 * right,
            1. / 2.,
            -top_left - 2 * top - top_right + left + 2 * center + right,
        )
    }

    /// Computes and returns the normal vector for the bottom right pixel for light filters.
    #[inline]
    pub fn bottom_right(surface: &SharedImageSurface, bounds: IRect) -> Normal {
        // Surface needs to be at least 2×2.
        assert!(bounds.width() >= 2);
        assert!(bounds.height() >= 2);

        let get = |x, y| i16::from(surface.get_pixel(x, y).a);
        let (x, y) = (bounds.x1 as u32 - 1, bounds.y1 as u32 - 1);

        let top_left = get(x - 1, y - 1);
        let top = get(x, y - 1);
        let left = get(x - 1, y);
        let center = get(x, y);

        Self::new(
            2. / 3.,
            -top_left + top - 2 * left + 2 * center,
            2. / 3.,
            -top_left - 2 * top + left + 2 * center,
        )
    }
}
