//! Lighting filters and light nodes.

use float_cmp::approx_eq;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use nalgebra::{Vector2, Vector3};
use num_traits::identities::Zero;
use rayon::prelude::*;
use std::cmp::max;

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::filters::{
    bounds::BoundsBuilder,
    context::{FilterContext, FilterOutput},
    FilterEffect, FilterError, FilterResolveError, Input, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::paint_server::resolve_color;
use crate::parsers::{NonNegative, NumberOptionalNumber, ParseValue};
use crate::properties::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::{
    shared_surface::{ExclusiveImageSurface, SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt, Pixel,
};
use crate::transform::Transform;
use crate::unit_interval::UnitInterval;
use crate::util::clamp;
use crate::xml::Attributes;

/// The `feDiffuseLighting` filter primitives.
#[derive(Default)]
pub struct FeDiffuseLighting {
    base: Primitive,
    params: DiffuseLightingParams,
}

#[derive(Clone)]
pub struct DiffuseLightingParams {
    in1: Input,
    surface_scale: f64,
    kernel_unit_length: Option<(f64, f64)>,
    diffuse_constant: f64,
}

impl Default for DiffuseLightingParams {
    fn default() -> Self {
        Self {
            in1: Default::default(),
            surface_scale: 1.0,
            kernel_unit_length: None,
            diffuse_constant: 1.0,
        }
    }
}

/// The `feSpecularLighting` filter primitives.
#[derive(Default)]
pub struct FeSpecularLighting {
    base: Primitive,
    params: SpecularLightingParams,
}

#[derive(Clone)]
pub struct SpecularLightingParams {
    in1: Input,
    surface_scale: f64,
    kernel_unit_length: Option<(f64, f64)>,
    specular_constant: f64,
    specular_exponent: f64,
}

impl Default for SpecularLightingParams {
    fn default() -> Self {
        Self {
            in1: Default::default(),
            surface_scale: 1.0,
            kernel_unit_length: None,
            specular_constant: 1.0,
            specular_exponent: 1.0,
        }
    }
}

/// Resolved `feDiffuseLighting` primitive for rendering.
pub struct DiffuseLighting {
    params: DiffuseLightingParams,
    light: Light,
}

/// Resolved `feSpecularLighting` primitive for rendering.
pub struct SpecularLighting {
    params: SpecularLightingParams,
    light: Light,
}

/// A light source before applying affine transformations, straight out of the SVG.
#[derive(Debug, PartialEq)]
enum UntransformedLightSource {
    Distant(FeDistantLight),
    Point(FePointLight),
    Spot(FeSpotLight),
}

/// A light source with affine transformations applied.
enum LightSource {
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

impl UntransformedLightSource {
    fn transform(&self, paffine: Transform) -> LightSource {
        match *self {
            UntransformedLightSource::Distant(ref l) => l.transform(),
            UntransformedLightSource::Point(ref l) => l.transform(paffine),
            UntransformedLightSource::Spot(ref l) => l.transform(paffine),
        }
    }
}

struct Light {
    source: UntransformedLightSource,
    lighting_color: cssparser::RGBA,
    color_interpolation_filters: ColorInterpolationFilters,
}

impl Light {
    /// Returns the color and unit (or null) vector from the image sample to the light.
    #[inline]
    pub fn color_and_vector(
        &self,
        source: &LightSource,
        x: f64,
        y: f64,
        z: f64,
    ) -> (cssparser::RGBA, Vector3<f64>) {
        let vector = match *source {
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
        };

        let color = match *source {
            LightSource::Spot {
                direction,
                specular_exponent,
                limiting_cone_angle,
                ..
            } => {
                let minus_l_dot_s = -vector.dot(&direction);
                match limiting_cone_angle {
                    _ if minus_l_dot_s <= 0.0 => cssparser::RGBA::transparent(),
                    Some(a) if minus_l_dot_s < a.to_radians().cos() => {
                        cssparser::RGBA::transparent()
                    }
                    _ => {
                        let factor = minus_l_dot_s.powf(specular_exponent);
                        let compute = |x| (clamp(f64::from(x) * factor, 0.0, 255.0) + 0.5) as u8;

                        cssparser::RGBA {
                            red: compute(self.lighting_color.red),
                            green: compute(self.lighting_color.green),
                            blue: compute(self.lighting_color.blue),
                            alpha: 255,
                        }
                    }
                }
            }
            _ => self.lighting_color,
        };

        (color, vector)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeDistantLight {
    azimuth: f64,
    elevation: f64,
}

impl FeDistantLight {
    fn transform(&self) -> LightSource {
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FePointLight {
    x: f64,
    y: f64,
    z: f64,
}

impl FePointLight {
    fn transform(&self, paffine: Transform) -> LightSource {
        let (x, y) = paffine.transform_point(self.x, self.y);
        let z = transform_dist(paffine, self.z);

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

#[derive(Clone, Debug, Default, PartialEq)]
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
    fn transform(&self, paffine: Transform) -> LightSource {
        let (x, y) = paffine.transform_point(self.x, self.y);
        let z = transform_dist(paffine, self.z);
        let (points_at_x, points_at_y) =
            paffine.transform_point(self.points_at_x, self.points_at_y);
        let points_at_z = transform_dist(paffine, self.points_at_z);

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
                    self.limiting_cone_angle = attr.parse(value)?
                }

                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for FeSpotLight {}

/// Applies the `primitiveUnits` coordinate transformation to a non-x or y distance.
#[inline]
fn transform_dist(t: Transform, d: f64) -> f64 {
    d * (t.xx.powi(2) + t.yy.powi(2)).sqrt() / std::f64::consts::SQRT_2
}

impl SetAttributes for FeDiffuseLighting {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.params.in1 = self.base.parse_one_input(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "surfaceScale") => {
                    self.params.surface_scale = attr.parse(value)?
                }
                expanded_name!("", "kernelUnitLength") => {
                    let NumberOptionalNumber(NonNegative(x), NonNegative(y)) = attr.parse(value)?;
                    self.params.kernel_unit_length = Some((x, y));
                }
                expanded_name!("", "diffuseConstant") => {
                    let NonNegative(c) = attr.parse(value)?;
                    self.params.diffuse_constant = c;
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl DiffuseLighting {
    #[inline]
    fn compute_factor(&self, normal: Normal, light_vector: Vector3<f64>) -> f64 {
        let k = if normal.normal.is_zero() {
            // Common case of (0, 0, 1) normal.
            light_vector.z
        } else {
            let mut n = normal
                .normal
                .map(|x| f64::from(x) * self.params.surface_scale / 255.);
            n.component_mul_assign(&normal.factor);
            let normal = Vector3::new(n.x, n.y, 1.0);

            normal.dot(&light_vector) / normal.norm()
        };

        self.params.diffuse_constant * k
    }
}

impl SetAttributes for FeSpecularLighting {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.params.in1 = self.base.parse_one_input(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "surfaceScale") => {
                    self.params.surface_scale = attr.parse(value)?
                }
                expanded_name!("", "kernelUnitLength") => {
                    let NumberOptionalNumber(NonNegative(x), NonNegative(y)) = attr.parse(value)?;
                    self.params.kernel_unit_length = Some((x, y));
                }
                expanded_name!("", "specularConstant") => {
                    let NonNegative(c) = attr.parse(value)?;
                    self.params.specular_constant = c;
                }
                expanded_name!("", "specularExponent") => {
                    self.params.specular_exponent = attr.parse(value)?;
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl SpecularLighting {
    #[inline]
    fn compute_factor(&self, normal: Normal, light_vector: Vector3<f64>) -> f64 {
        let h = light_vector + Vector3::new(0.0, 0.0, 1.0);
        let h_norm = h.norm();

        if h_norm == 0.0 {
            return 0.0;
        }

        let n_dot_h = if normal.normal.is_zero() {
            // Common case of (0, 0, 1) normal.
            h.z / h_norm
        } else {
            let mut n = normal
                .normal
                .map(|x| f64::from(x) * self.params.surface_scale / 255.);
            n.component_mul_assign(&normal.factor);
            let normal = Vector3::new(n.x, n.y, 1.0);
            normal.dot(&h) / normal.norm() / h_norm
        };

        if approx_eq!(f64, self.params.specular_exponent, 1.0) {
            self.params.specular_constant * n_dot_h
        } else {
            self.params.specular_constant * n_dot_h.powf(self.params.specular_exponent)
        }
    }
}

macro_rules! impl_lighting_filter {
    ($lighting_type:ty, $params_name:ident, $alpha_func:ident) => {
        impl $params_name {
            pub fn render(
                &self,
                bounds_builder: BoundsBuilder,
                ctx: &FilterContext,
                acquired_nodes: &mut AcquiredNodes<'_>,
                draw_ctx: &mut DrawingCtx,
            ) -> Result<FilterOutput, FilterError> {
                let input_1 = ctx.get_input(
                    acquired_nodes,
                    draw_ctx,
                    &self.params.in1,
                    self.light.color_interpolation_filters,
                )?;
                let mut bounds: IRect = bounds_builder
                    .add_input(&input_1)
                    .compute(ctx)
                    .clipped
                    .into();
                let original_bounds = bounds;

                let scale = self
                    .params
                    .kernel_unit_length
                    .map(|(dx, dy)| ctx.paffine().transform_distance(dx, dy));

                let mut input_surface = input_1.surface().clone();

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

                let source = self.light.source.transform(ctx.paffine());

                let mut surface = ExclusiveImageSurface::new(
                    input_surface.width(),
                    input_surface.height(),
                    SurfaceType::from(self.light.color_interpolation_filters),
                )?;

                {
                    let output_stride = surface.stride() as usize;
                    let mut output_data = surface.data();
                    let output_slice = &mut *output_data;

                    let compute_output_pixel =
                        |output_slice: &mut [u8], base_y, x, y, normal: Normal| {
                            let pixel = input_surface.get_pixel(x, y);

                            let scaled_x = f64::from(x) * ox;
                            let scaled_y = f64::from(y) * oy;
                            let z = f64::from(pixel.a) / 255.0 * self.params.surface_scale;

                            let (color, vector) =
                                self.light.color_and_vector(&source, scaled_x, scaled_y, z);

                            // compute the factor just once for the three colors
                            let factor = self.compute_factor(normal, vector);
                            let compute =
                                |x| (clamp(factor * f64::from(x), 0.0, 255.0) + 0.5) as u8;

                            let r = compute(color.red);
                            let g = compute(color.green);
                            let b = compute(color.blue);
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

                Ok(FilterOutput { surface, bounds })
            }
        }

        impl FilterEffect for $lighting_type {
            fn resolve(
                &self,
                _acquired_nodes: &mut AcquiredNodes<'_>,
                node: &Node,
            ) -> Result<ResolvedPrimitive, FilterResolveError> {
                let mut sources = node.children().rev().filter(|c| {
                    c.is_element()
                        && matches!(
                            *c.borrow_element(),
                            Element::FeDistantLight(_)
                                | Element::FePointLight(_)
                                | Element::FeSpotLight(_)
                        )
                });

                let source_node = sources.next();
                if source_node.is_none() || sources.next().is_some() {
                    return Err(FilterResolveError::InvalidLightSourceCount);
                }

                let source_node = source_node.unwrap();
                let elt = source_node.borrow_element();

                if elt.is_in_error() {
                    return Err(FilterResolveError::ChildNodeInError);
                }

                let source = match *elt {
                    Element::FeDistantLight(ref l) => {
                        UntransformedLightSource::Distant(l.element_impl.clone())
                    }
                    Element::FePointLight(ref l) => {
                        UntransformedLightSource::Point(l.element_impl.clone())
                    }
                    Element::FeSpotLight(ref l) => {
                        UntransformedLightSource::Spot(l.element_impl.clone())
                    }
                    _ => unreachable!(),
                };

                let cascaded = CascadedValues::new_from_node(node);
                let values = cascaded.get();

                Ok(ResolvedPrimitive {
                    primitive: self.base.clone(),
                    params: PrimitiveParams::$params_name($params_name {
                        params: self.params.clone(),
                        light: Light {
                            source,
                            lighting_color: resolve_color(
                                &values.lighting_color().0,
                                UnitInterval::clamp(1.0),
                                values.color().0,
                            ),
                            color_interpolation_filters: values.color_interpolation_filters(),
                        },
                    }),
                })
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

impl_lighting_filter!(FeDiffuseLighting, DiffuseLighting, diffuse_alpha);

impl_lighting_filter!(FeSpecularLighting, SpecularLighting, specular_alpha);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn extracts_light_source() {
        let document = Document::load_from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <filter id="filter">
    <feDiffuseLighting id="diffuse_distant">
      <feDistantLight azimuth="0.0" elevation="45.0"/>
    </feDiffuseLighting>

    <feSpecularLighting id="specular_point">
      <fePointLight x="1.0" y="2.0" z="3.0"/>
    </feSpecularLighting>

    <feDiffuseLighting id="diffuse_spot">
      <feSpotLight x="1.0" y="2.0" z="3.0"
                   pointsAtX="4.0" pointsAtY="5.0" pointsAtZ="6.0"
                   specularExponent="7.0" limitingConeAngle="8.0"/>
    </feDiffuseLighting>
  </filter>
</svg>
"#,
        );
        let mut acquired_nodes = AcquiredNodes::new(&document);

        let node = document.lookup_internal_node("diffuse_distant").unwrap();
        let lighting = borrow_element_as!(node, FeDiffuseLighting);
        let ResolvedPrimitive { params, .. } =
            lighting.resolve(&mut acquired_nodes, &node).unwrap();
        let diffuse_lighting = match params {
            PrimitiveParams::DiffuseLighting(l) => l,
            _ => unreachable!(),
        };
        assert_eq!(
            diffuse_lighting.light.source,
            UntransformedLightSource::Distant(FeDistantLight {
                azimuth: 0.0,
                elevation: 45.0,
            })
        );

        let node = document.lookup_internal_node("specular_point").unwrap();
        let lighting = borrow_element_as!(node, FeSpecularLighting);
        let ResolvedPrimitive { params, .. } =
            lighting.resolve(&mut acquired_nodes, &node).unwrap();
        let specular_lighting = match params {
            PrimitiveParams::SpecularLighting(l) => l,
            _ => unreachable!(),
        };
        assert_eq!(
            specular_lighting.light.source,
            UntransformedLightSource::Point(FePointLight {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            })
        );

        let node = document.lookup_internal_node("diffuse_spot").unwrap();
        let lighting = borrow_element_as!(node, FeDiffuseLighting);
        let ResolvedPrimitive { params, .. } =
            lighting.resolve(&mut acquired_nodes, &node).unwrap();
        let diffuse_lighting = match params {
            PrimitiveParams::DiffuseLighting(l) => l,
            _ => unreachable!(),
        };
        assert_eq!(
            diffuse_lighting.light.source,
            UntransformedLightSource::Spot(FeSpotLight {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                points_at_x: 4.0,
                points_at_y: 5.0,
                points_at_z: 6.0,
                specular_exponent: 7.0,
                limiting_cone_angle: Some(8.0),
            })
        );
    }
}
