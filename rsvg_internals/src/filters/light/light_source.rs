use cssparser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use nalgebra::Vector3;

use crate::filters::context::FilterContext;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{ParseValue};
use crate::property_bag::PropertyBag;
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

impl NodeTrait for FeDistantLight {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "azimuth") => self.azimuth = attr.parse(value)?,
                expanded_name!(svg "elevation") => self.elevation = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

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

impl NodeTrait for FePointLight {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "z") => self.z = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

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

impl NodeTrait for FeSpotLight {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "z") => self.z = attr.parse(value)?,
                expanded_name!(svg "pointsAtX") => self.points_at_x = attr.parse(value)?,
                expanded_name!(svg "pointsAtY") => self.points_at_y = attr.parse(value)?,
                expanded_name!(svg "pointsAtZ") => self.points_at_z = attr.parse(value)?,

                expanded_name!(svg "specularExponent") => {
                    self.specular_exponent = attr.parse(value)?
                }

                expanded_name!(svg "limitingConeAngle") => {
                    self.limiting_cone_angle = Some(attr.parse(value)?)
                }

                _ => (),
            }
        }

        Ok(())
    }
}
