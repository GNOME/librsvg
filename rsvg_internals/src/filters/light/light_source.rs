use cssparser;
use markup5ever::local_name;
use nalgebra::Vector3;

use crate::error::AttributeResultExt;
use crate::filters::context::FilterContext;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers;
use crate::property_bag::PropertyBag;
use crate::util::clamp;

/// A light source node (`feDistantLight`, `fePointLight` or `feSpotLight`).
#[derive(Clone)]
pub enum LightSource {
    Distant {
        azimuth: f64,
        elevation: f64,
    },
    Point {
        x: f64,
        y: f64,
        z: f64,
    },
    Spot {
        x: f64,
        y: f64,
        z: f64,
        points_at_x: f64,
        points_at_y: f64,
        points_at_z: f64,
        specular_exponent: f64,
        limiting_cone_angle: Option<f64>,
    },
}

/// A light source node with affine transformations applied.
pub enum TransformedLightSource {
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
    /// Constructs a new `feDistantLight` with empty properties.
    #[inline]
    pub fn new_distant_light() -> LightSource {
        LightSource::Distant {
            azimuth: 0.0,
            elevation: 0.0,
        }
    }

    /// Constructs a new `fePointLight` with empty properties.
    #[inline]
    pub fn new_point_light() -> LightSource {
        LightSource::Point {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Constructs a new `feSpotLight` with empty properties.
    #[inline]
    pub fn new_spot_light() -> LightSource {
        LightSource::Spot {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            points_at_x: 0.0,
            points_at_y: 0.0,
            points_at_z: 0.0,
            specular_exponent: 0.0,
            limiting_cone_angle: None,
        }
    }

    /// Returns a `TransformedLightSource` according to the given `FilterContext`.
    #[inline]
    pub fn transform(&self, ctx: &FilterContext) -> TransformedLightSource {
        match *self {
            LightSource::Distant { azimuth, elevation } => {
                TransformedLightSource::Distant { azimuth, elevation }
            }
            LightSource::Point { x, y, z } => {
                let (x, y) = ctx.paffine().transform_point(x, y);
                let z = ctx.transform_dist(z);

                TransformedLightSource::Point {
                    origin: Vector3::new(x, y, z),
                }
            }
            LightSource::Spot {
                x,
                y,
                z,
                points_at_x,
                points_at_y,
                points_at_z,
                specular_exponent,
                limiting_cone_angle,
            } => {
                let (x, y) = ctx.paffine().transform_point(x, y);
                let z = ctx.transform_dist(z);
                let (points_at_x, points_at_y) =
                    ctx.paffine().transform_point(points_at_x, points_at_y);
                let points_at_z = ctx.transform_dist(points_at_z);

                let origin = Vector3::new(x, y, z);
                let mut direction = Vector3::new(points_at_x, points_at_y, points_at_z) - origin;
                let _ = direction.try_normalize_mut(0.0);

                TransformedLightSource::Spot {
                    origin,
                    direction,
                    specular_exponent,
                    limiting_cone_angle,
                }
            }
        }
    }
}

impl TransformedLightSource {
    /// Returns the unit (or null) vector from the image sample to the light.
    #[inline]
    pub fn vector(&self, x: f64, y: f64, z: f64) -> Vector3<f64> {
        match self {
            TransformedLightSource::Distant { azimuth, elevation } => {
                let azimuth = azimuth.to_radians();
                let elevation = elevation.to_radians();
                Vector3::new(
                    azimuth.cos() * elevation.cos(),
                    azimuth.sin() * elevation.cos(),
                    elevation.sin(),
                )
            }
            TransformedLightSource::Point { origin }
            | TransformedLightSource::Spot { origin, .. } => {
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
            TransformedLightSource::Spot {
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

impl NodeTrait for LightSource {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match *self {
                LightSource::Distant {
                    ref mut azimuth,
                    ref mut elevation,
                } => match attr {
                    local_name!("azimuth") => *azimuth = parsers::number(value).attribute(attr)?,
                    local_name!("elevation") => {
                        *elevation = parsers::number(value).attribute(attr)?
                    }
                    _ => (),
                },
                LightSource::Point {
                    ref mut x,
                    ref mut y,
                    ref mut z,
                } => match attr {
                    local_name!("x") => *x = parsers::number(value).attribute(attr)?,
                    local_name!("y") => *y = parsers::number(value).attribute(attr)?,
                    local_name!("z") => *z = parsers::number(value).attribute(attr)?,
                    _ => (),
                },
                LightSource::Spot {
                    ref mut x,
                    ref mut y,
                    ref mut z,
                    ref mut points_at_x,
                    ref mut points_at_y,
                    ref mut points_at_z,
                    ref mut specular_exponent,
                    ref mut limiting_cone_angle,
                } => match attr {
                    local_name!("x") => *x = parsers::number(value).attribute(attr)?,
                    local_name!("y") => *y = parsers::number(value).attribute(attr)?,
                    local_name!("z") => *z = parsers::number(value).attribute(attr)?,
                    local_name!("pointsAtX") => {
                        *points_at_x = parsers::number(value).attribute(attr)?
                    }
                    local_name!("pointsAtY") => {
                        *points_at_y = parsers::number(value).attribute(attr)?
                    }
                    local_name!("pointsAtZ") => {
                        *points_at_z = parsers::number(value).attribute(attr)?
                    }
                    local_name!("specularExponent") => {
                        *specular_exponent = parsers::number(value).attribute(attr)?
                    }
                    local_name!("limitingConeAngle") => {
                        *limiting_cone_angle = Some(parsers::number(value).attribute(attr)?)
                    }
                    _ => (),
                },
            }
        }

        Ok(())
    }
}
