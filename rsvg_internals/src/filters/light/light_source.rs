use std::cell::Cell;

use cairo::MatrixTrait;
use cssparser;
use rulinalg::vector::Vector;

use attributes::Attribute;
use error::NodeError;
use filters::{context::FilterContext, light::normalize};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use surface_utils::shared_surface::SharedImageSurface;
use util::clamp;

/// A light source node (`feDistantLight`, `fePointLight` or `feSpotLight`).
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

    /// Returns the unit (or null) vector from the image sample to the light.
    #[inline]
    pub fn vector(
        &self,
        surface: &SharedImageSurface,
        x: u32,
        y: u32,
        surface_scale: f64,
        ctx: &FilterContext,
    ) -> Vector<f64> {
        match self {
            LightSource::Distant { azimuth, elevation } => {
                let azimuth = azimuth.get().to_radians();
                let elevation = elevation.get().to_radians();
                vector![
                    azimuth.cos() * elevation.cos(),
                    azimuth.sin() * elevation.cos(),
                    elevation.sin()
                ]
            }
            LightSource::Point {
                x: light_x,
                y: light_y,
                z: light_z,
            }
            | LightSource::Spot {
                x: light_x,
                y: light_y,
                z: light_z,
                ..
            } => {
                let (light_x_, light_y_) =
                    ctx.paffine().transform_point(light_x.get(), light_y.get());
                let light_z_ = ctx.transform_dist(light_z.get());

                let z = f64::from(surface.get_pixel(x, y).a) / 255.0;

                let mut v = vector![
                    light_x_ - f64::from(x),
                    light_y_ - f64::from(y),
                    light_z_ - z * surface_scale
                ];
                let _ = normalize(&mut v);
                v
            }
        }
    }

    /// Returns the color of the light.
    #[inline]
    pub fn color(
        &self,
        lighting_color: cssparser::RGBA,
        light_vector: &Vector<f64>,
        ctx: &FilterContext,
    ) -> cssparser::RGBA {
        match self {
            LightSource::Spot {
                x: light_x,
                y: light_y,
                z: light_z,
                points_at_x,
                points_at_y,
                points_at_z,
                specular_exponent,
                limiting_cone_angle,
                ..
            } => {
                let (light_x, light_y) =
                    ctx.paffine().transform_point(light_x.get(), light_y.get());
                let light_z = ctx.transform_dist(light_z.get());
                let (points_at_x, points_at_y) = ctx
                    .paffine()
                    .transform_point(points_at_x.get(), points_at_y.get());
                let points_at_z = ctx.transform_dist(points_at_z.get());

                let mut s = vector![
                    points_at_x - light_x,
                    points_at_y - light_y,
                    points_at_z - light_z
                ];
                if normalize(&mut s).is_err() {
                    return cssparser::RGBA::transparent();
                }

                let minus_l_dot_s = -light_vector.dot(&s);
                if minus_l_dot_s <= 0.0 {
                    return cssparser::RGBA::transparent();
                }

                if let Some(limiting_cone_angle) = limiting_cone_angle.get() {
                    if minus_l_dot_s < limiting_cone_angle.to_radians().cos() {
                        return cssparser::RGBA::transparent();
                    }
                }

                let factor = minus_l_dot_s.powf(specular_exponent.get());
                let compute = |x| clamp(f64::from(x) * factor, 0.0, 255.0).round() as u8;

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
        pbag: &PropertyBag,
    ) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match self {
                LightSource::Distant {
                    ref azimuth,
                    ref elevation,
                } => match attr {
                    Attribute::Azimuth => azimuth.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::Elevation => elevation.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    _ => (),
                },
                LightSource::Point {
                    ref x,
                    ref y,
                    ref z,
                } => match attr {
                    Attribute::X => x.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::Y => y.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::Z => z.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
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
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::Y => y.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::Z => z.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::PointsAtX => points_at_x.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::PointsAtY => points_at_y.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::PointsAtZ => points_at_z.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::SpecularExponent => specular_exponent.set(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    ),
                    Attribute::LimitingConeAngle => limiting_cone_angle.set(Some(
                        parsers::number(value).map_err(|err| NodeError::parse_error(attr, err))?,
                    )),
                    _ => (),
                },
            }
        }

        Ok(())
    }
}
