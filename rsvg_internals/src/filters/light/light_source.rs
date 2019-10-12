use cssparser;
use markup5ever::local_name;
use nalgebra::Vector3;

use crate::error::AttributeResultExt;
use crate::filters::context::FilterContext;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers;
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
pub struct DistantLight {
    azimuth: f64,
    elevation: f64,
}

impl DistantLight {
    pub fn transform(&self, _ctx: &FilterContext) -> LightSource {
        LightSource::Distant {
            azimuth: self.azimuth,
            elevation: self.elevation,
        }
    }
}

impl NodeTrait for DistantLight {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("azimuth") => self.azimuth = parsers::number(value).attribute(attr)?,
                local_name!("elevation") => {
                    self.elevation = parsers::number(value).attribute(attr)?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct PointLight {
    x: f64,
    y: f64,
    z: f64,
}

impl PointLight {
    pub fn transform(&self, ctx: &FilterContext) -> LightSource {
        let (x, y) = ctx.paffine().transform_point(self.x, self.y);
        let z = ctx.transform_dist(self.z);

        LightSource::Point {
            origin: Vector3::new(x, y, z),
        }
    }
}

impl NodeTrait for PointLight {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("x") => self.x = parsers::number(value).attribute(attr)?,
                local_name!("y") => self.y = parsers::number(value).attribute(attr)?,
                local_name!("z") => self.z = parsers::number(value).attribute(attr)?,
                _ => (),
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct SpotLight {
    x: f64,
    y: f64,
    z: f64,
    points_at_x: f64,
    points_at_y: f64,
    points_at_z: f64,
    specular_exponent: f64,
    limiting_cone_angle: Option<f64>,
}

impl SpotLight {
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

impl NodeTrait for SpotLight {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("x") => self.x = parsers::number(value).attribute(attr)?,
                local_name!("y") => self.y = parsers::number(value).attribute(attr)?,
                local_name!("z") => self.z = parsers::number(value).attribute(attr)?,
                local_name!("pointsAtX") => {
                    self.points_at_x = parsers::number(value).attribute(attr)?
                }
                local_name!("pointsAtY") => {
                    self.points_at_y = parsers::number(value).attribute(attr)?
                }
                local_name!("pointsAtZ") => {
                    self.points_at_z = parsers::number(value).attribute(attr)?
                }
                local_name!("specularExponent") => {
                    self.specular_exponent = parsers::number(value).attribute(attr)?
                }
                local_name!("limitingConeAngle") => {
                    self.limiting_cone_angle = Some(parsers::number(value).attribute(attr)?)
                }
                _ => (),
            }
        }

        Ok(())
    }
}
