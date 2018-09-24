use std::cell::Cell;

use cairo::MatrixTrait;
use cssparser;
use nalgebra::Vector3;

use attributes::Attribute;
use error::NodeError;
use filters::context::FilterContext;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use util::clamp;

/// A light source node (`feDistantLight`, `fePointLight` or `feSpotLight`).
#[derive(Clone)]
pub enum LightSource {
    Distant {
        azimuth: Cell<f64>,
        elevation: Cell<f64>,
    },
    Point {
        x: Cell<f64>,
        y: Cell<f64>,
        z: Cell<f64>,
    },
    Spot {
        x: Cell<f64>,
        y: Cell<f64>,
        z: Cell<f64>,
        points_at_x: Cell<f64>,
        points_at_y: Cell<f64>,
        points_at_z: Cell<f64>,
        specular_exponent: Cell<f64>,
        limiting_cone_angle: Cell<Option<f64>>,
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
            azimuth: Cell::new(0.0),
            elevation: Cell::new(0.0),
        }
    }

    /// Constructs a new `fePointLight` with empty properties.
    #[inline]
    pub fn new_point_light() -> LightSource {
        LightSource::Point {
            x: Cell::new(0.0),
            y: Cell::new(0.0),
            z: Cell::new(0.0),
        }
    }

    /// Constructs a new `feSpotLight` with empty properties.
    #[inline]
    pub fn new_spot_light() -> LightSource {
        LightSource::Spot {
            x: Cell::new(0.0),
            y: Cell::new(0.0),
            z: Cell::new(0.0),
            points_at_x: Cell::new(0.0),
            points_at_y: Cell::new(0.0),
            points_at_z: Cell::new(0.0),
            specular_exponent: Cell::new(0.0),
            limiting_cone_angle: Cell::new(None),
        }
    }

    /// Returns a `TransformedLightSource` according to the given `FilterContext`.
    #[inline]
    pub fn transform(&self, ctx: &FilterContext) -> TransformedLightSource {
        match self {
            LightSource::Distant { azimuth, elevation } => TransformedLightSource::Distant {
                azimuth: azimuth.get(),
                elevation: elevation.get(),
            },
            LightSource::Point { x, y, z } => {
                let (x, y) = ctx.paffine().transform_point(x.get(), y.get());
                let z = ctx.transform_dist(z.get());

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
                let (x, y) = ctx.paffine().transform_point(x.get(), y.get());
                let z = ctx.transform_dist(z.get());
                let (points_at_x, points_at_y) = ctx
                    .paffine()
                    .transform_point(points_at_x.get(), points_at_y.get());
                let points_at_z = ctx.transform_dist(points_at_z.get());

                let origin = Vector3::new(x, y, z);
                let mut direction = Vector3::new(points_at_x, points_at_y, points_at_z) - origin;
                let _ = direction.try_normalize_mut(0.0);

                TransformedLightSource::Spot {
                    origin,
                    direction,
                    specular_exponent: specular_exponent.get(),
                    limiting_cone_angle: limiting_cone_angle.get(),
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
    fn set_atts(
        &self,
        _node: &RsvgNode,
        _handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match self {
                LightSource::Distant {
                    ref azimuth,
                    ref elevation,
                } => match attr {
                    Attribute::Azimuth => azimuth.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::Elevation => elevation.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    _ => (),
                },
                LightSource::Point {
                    ref x,
                    ref y,
                    ref z,
                } => match attr {
                    Attribute::X => x.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::Y => y.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::Z => z.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    _ => (),
                },
                LightSource::Spot {
                    ref x,
                    ref y,
                    ref z,
                    ref points_at_x,
                    ref points_at_y,
                    ref points_at_z,
                    ref specular_exponent,
                    ref limiting_cone_angle,
                } => match attr {
                    Attribute::X => x.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::Y => y.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::Z => z.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::PointsAtX => points_at_x.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::PointsAtY => points_at_y.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::PointsAtZ => points_at_z.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::SpecularExponent => specular_exponent.set(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    ),
                    Attribute::LimitingConeAngle => limiting_cone_angle.set(Some(
                        parsers::number(value)
                            .map_err(|err| NodeError::attribute_error(attr, err))?,
                    )),
                    _ => (),
                },
            }
        }

        Ok(())
    }
}
