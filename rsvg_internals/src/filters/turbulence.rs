use std::cell::Cell;

use cairo::{self, ImageSurface, MatrixTrait};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{self, ParseError};
use property_bag::PropertyBag;
use state::ColorInterpolationFilters;
use surface_utils::{
    shared_surface::{SharedImageSurface, SurfaceType},
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{Filter, FilterError, Primitive};

/// Enumeration of the tile stitching modes.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum StitchTiles {
    Stitch,
    NoStitch,
}

/// Enumeration of the noise types.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum NoiseType {
    FractalNoise,
    Turbulence,
}

/// The `feTurbulence` filter primitive.
pub struct Turbulence {
    base: Primitive,
    base_frequency: Cell<(f64, f64)>,
    num_octaves: Cell<i32>,
    seed: Cell<i32>,
    stitch_tiles: Cell<StitchTiles>,
    type_: Cell<NoiseType>,
}

impl Turbulence {
    /// Constructs a new `Turbulence` with empty properties.
    #[inline]
    pub fn new() -> Turbulence {
        Turbulence {
            base: Primitive::new::<Self>(),
            base_frequency: Cell::new((0.0, 0.0)),
            num_octaves: Cell::new(1),
            seed: Cell::new(0),
            stitch_tiles: Cell::new(StitchTiles::NoStitch),
            type_: Cell::new(NoiseType::Turbulence),
        }
    }
}

impl NodeTrait for Turbulence {
    #[inline]
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::BaseFrequency => self.base_frequency.set(
                    parsers::number_optional_number(value)
                        .map_err(|err| NodeError::attribute_error(attr, err))
                        .and_then(|(x, y)| {
                            if x >= 0.0 && y >= 0.0 {
                                Ok((x, y))
                            } else {
                                Err(NodeError::value_error(attr, "values can't be negative"))
                            }
                        })?,
                ),
                Attribute::NumOctaves => self.num_octaves.set(
                    parsers::integer(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                // Yes, seed needs to be parsed as a number and then truncated.
                Attribute::Seed => self.seed.set(
                    parsers::number(value)
                        .map(|x| {
                            clamp(
                                x.trunc(),
                                f64::from(i32::min_value()),
                                f64::from(i32::max_value()),
                            ) as i32
                        })
                        .map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                Attribute::StitchTiles => self.stitch_tiles.set(StitchTiles::parse(attr, value)?),
                Attribute::Type => self.type_.set(NoiseType::parse(attr, value)?),
                _ => (),
            }
        }

        Ok(())
    }
}

// Produces results in the range [1, 2**31 - 2].
// Algorithm is: r = (a * r) mod m
// where a = 16807 and m = 2**31 - 1 = 2147483647
// See [Park & Miller], CACM vol. 31 no. 10 p. 1195, Oct. 1988
// To test: the algorithm should produce the result 1043618065
// as the 10,000th generated number if the original seed is 1.
const RAND_M: i32 = 2147483647; // 2**31 - 1
const RAND_A: i32 = 16807; // 7**5; primitive root of m
const RAND_Q: i32 = 127773; // m / a
const RAND_R: i32 = 2836; // m % a

fn setup_seed(mut seed: i32) -> i32 {
    if seed <= 0 {
        seed = -(seed % (RAND_M - 1)) + 1;
    }
    if seed > RAND_M - 1 {
        seed = RAND_M - 1;
    }
    seed
}

fn random(seed: i32) -> i32 {
    let mut result = RAND_A * (seed % RAND_Q) - RAND_R * (seed / RAND_Q);
    if result <= 0 {
        result += RAND_M;
    }
    result
}

const B_SIZE: usize = 0x100;
const PERLIN_N: i32 = 0x1000;

#[derive(Clone, Copy)]
struct NoiseGenerator {
    base_frequency: (f64, f64),
    num_octaves: i32,
    stitch_tiles: StitchTiles,
    type_: NoiseType,

    tile_width: f64,
    tile_height: f64,

    lattice_selector: [usize; B_SIZE + B_SIZE + 2],
    gradient: [[[f64; 2]; B_SIZE + B_SIZE + 2]; 4],
}

#[derive(Clone, Copy)]
struct StitchInfo {
    width: usize, // How much to subtract to wrap for stitching.
    height: usize,
    wrap_x: usize, // Minimum value to wrap.
    wrap_y: usize,
}

impl NoiseGenerator {
    fn new(
        seed: i32,
        base_frequency: (f64, f64),
        num_octaves: i32,
        type_: NoiseType,
        stitch_tiles: StitchTiles,
        tile_width: f64,
        tile_height: f64,
    ) -> Self {
        let mut rv = Self {
            base_frequency,
            num_octaves,
            type_,
            stitch_tiles,

            tile_width,
            tile_height,

            lattice_selector: [0; B_SIZE + B_SIZE + 2],
            gradient: [[[0.0; 2]; B_SIZE + B_SIZE + 2]; 4],
        };

        let mut seed = setup_seed(seed);

        for k in 0..4 {
            for i in 0..B_SIZE {
                rv.lattice_selector[i] = i;
                for j in 0..2 {
                    seed = random(seed);
                    rv.gradient[k][i][j] =
                        ((seed % (B_SIZE + B_SIZE) as i32) - B_SIZE as i32) as f64 / B_SIZE as f64;
                }
                let s = (rv.gradient[k][i][0] * rv.gradient[k][i][0]
                    + rv.gradient[k][i][1] * rv.gradient[k][i][1])
                    .sqrt();
                rv.gradient[k][i][0] /= s;
                rv.gradient[k][i][1] /= s;
            }
        }
        for i in (1..B_SIZE).rev() {
            let k = rv.lattice_selector[i];
            seed = random(seed);
            let j = seed as usize % B_SIZE;
            rv.lattice_selector[i] = rv.lattice_selector[j];
            rv.lattice_selector[j] = k;
        }
        for i in 0..B_SIZE + 2 {
            rv.lattice_selector[B_SIZE + i] = rv.lattice_selector[i];
            for k in 0..4 {
                for j in 0..2 {
                    rv.gradient[k][B_SIZE + i][j] = rv.gradient[k][i][j];
                }
            }
        }

        rv
    }

    fn noise2(&self, color_channel: usize, vec: [f64; 2], stitch_info: Option<StitchInfo>) -> f64 {
        const BM: usize = 0xff;

        let s_curve = |t| t * t * (3. - 2. * t);
        let lerp = |t, a, b| a + t * (b - a);

        let t = vec[0] + f64::from(PERLIN_N);
        let mut bx0 = t as usize;
        let mut bx1 = bx0 + 1;
        let rx0 = t.fract();
        let rx1 = rx0 - 1.0;
        let t = vec[1] + f64::from(PERLIN_N);
        let mut by0 = t as usize;
        let mut by1 = by0 + 1;
        let ry0 = t.fract();
        let ry1 = ry0 - 1.0;

        // If stitching, adjust lattice points accordingly.
        if let Some(stitch_info) = stitch_info {
            if bx0 >= stitch_info.wrap_x {
                bx0 -= stitch_info.width;
            }
            if bx1 >= stitch_info.wrap_x {
                bx1 -= stitch_info.width;
            }
            if by0 >= stitch_info.wrap_y {
                by0 -= stitch_info.height;
            }
            if by1 >= stitch_info.wrap_y {
                by1 -= stitch_info.height;
            }
        }
        bx0 &= BM;
        bx1 &= BM;
        by0 &= BM;
        by1 &= BM;
        let i = self.lattice_selector[bx0];
        let j = self.lattice_selector[bx1];
        let b00 = self.lattice_selector[i + by0];
        let b10 = self.lattice_selector[j + by0];
        let b01 = self.lattice_selector[i + by1];
        let b11 = self.lattice_selector[j + by1];
        let sx = f64::from(s_curve(rx0));
        let sy = f64::from(s_curve(ry0));
        let q = self.gradient[color_channel][b00];
        let u = rx0 * q[0] + ry0 * q[1];
        let q = self.gradient[color_channel][b10];
        let v = rx1 * q[0] + ry0 * q[1];
        let a = lerp(sx, u, v);
        let q = self.gradient[color_channel][b01];
        let u = rx0 * q[0] + ry1 * q[1];
        let q = self.gradient[color_channel][b11];
        let v = rx1 * q[0] + ry1 * q[1];
        let b = lerp(sx, u, v);
        lerp(sy, a, b)
    }

    fn turbulence(&self, color_channel: usize, point: [f64; 2], tile_x: f64, tile_y: f64) -> f64 {
        let mut stitch_info = None;
        let mut base_frequency = self.base_frequency;

        // Adjust the base frequencies if necessary for stitching.
        if self.stitch_tiles == StitchTiles::Stitch {
            // When stitching tiled turbulence, the frequencies must be adjusted
            // so that the tile borders will be continuous.
            if base_frequency.0 != 0.0 {
                let freq_lo = (self.tile_width * base_frequency.0).floor() / self.tile_width;
                let freq_hi = (self.tile_width * base_frequency.0).ceil() / self.tile_width;
                if base_frequency.0 / freq_lo < freq_hi / base_frequency.0 {
                    base_frequency.0 = freq_lo;
                } else {
                    base_frequency.0 = freq_hi;
                }
            }
            if base_frequency.1 != 0.0 {
                let freq_lo = (self.tile_height * base_frequency.1).floor() / self.tile_height;
                let freq_hi = (self.tile_height * base_frequency.1).ceil() / self.tile_height;
                if base_frequency.1 / freq_lo < freq_hi / base_frequency.1 {
                    base_frequency.1 = freq_lo;
                } else {
                    base_frequency.1 = freq_hi;
                }
            }

            // Set up initial stitch values.
            let width = (self.tile_width * base_frequency.0 + 0.5) as usize;
            let height = (self.tile_height * base_frequency.1 + 0.5) as usize;
            stitch_info = Some(StitchInfo {
                width,
                wrap_x: (tile_x * base_frequency.0) as usize + PERLIN_N as usize + width,
                height,
                wrap_y: (tile_y * base_frequency.1) as usize + PERLIN_N as usize + height,
            });
        }

        let mut sum = 0.0;
        let mut vec = [point[0] * base_frequency.0, point[1] * base_frequency.1];
        let mut ratio = 1.0;
        for _ in 0..self.num_octaves {
            if self.type_ == NoiseType::FractalNoise {
                sum += self.noise2(color_channel, vec, stitch_info) / ratio;
            } else {
                sum += (self.noise2(color_channel, vec, stitch_info)).abs() / ratio;
            }
            vec[0] *= 2.0;
            vec[1] *= 2.0;
            ratio *= 2.0;
            if let Some(stitch_info) = stitch_info.as_mut() {
                // Update stitch values. Subtracting PerlinN before the multiplication and
                // adding it afterward simplifies to subtracting it once.
                stitch_info.width *= 2;
                stitch_info.wrap_x = 2 * stitch_info.wrap_x - PERLIN_N as usize;
                stitch_info.height *= 2;
                stitch_info.wrap_y = 2 * stitch_info.wrap_y - PERLIN_N as usize;
            }
        }
        sum
    }
}

impl Filter for Turbulence {
    fn render(
        &self,
        node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError> {
        let bounds = self.base.get_bounds(ctx).into_irect(draw_ctx);

        let mut affine = ctx.paffine();
        affine.invert();

        let type_ = self.type_.get();
        let noise_generator = NoiseGenerator::new(
            self.seed.get(),
            self.base_frequency.get(),
            self.num_octaves.get(),
            type_,
            self.stitch_tiles.get(),
            f64::from(bounds.x1 - bounds.x0),
            f64::from(bounds.y1 - bounds.y0),
        );

        let mut output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for y in bounds.y0..bounds.y1 {
                for x in bounds.x0..bounds.x1 {
                    let point = affine.transform_point(f64::from(x), f64::from(y));
                    let point = [point.0, point.1];

                    let generate = |color_channel| {
                        let v = noise_generator.turbulence(
                            color_channel,
                            point,
                            f64::from(x - bounds.x0),
                            f64::from(y - bounds.y0),
                        );

                        let v = match type_ {
                            NoiseType::FractalNoise => (v * 255.0 + 255.0) / 2.0,
                            NoiseType::Turbulence => v * 255.0,
                        };

                        (clamp(v, 0.0, 255.0) + 0.5) as u8
                    };

                    let pixel = Pixel {
                        r: generate(0),
                        g: generate(1),
                        b: generate(2),
                        a: generate(3),
                    }
                    .premultiply();

                    output_data.set_pixel(output_stride, pixel, x as u32, y as u32);
                }
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

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, surface_type)?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}

impl StitchTiles {
    fn parse(attr: Attribute, s: &str) -> Result<Self, NodeError> {
        match s {
            "stitch" => Ok(StitchTiles::Stitch),
            "noStitch" => Ok(StitchTiles::NoStitch),
            _ => Err(NodeError::parse_error(
                attr,
                ParseError::new("invalid value"),
            )),
        }
    }
}

impl NoiseType {
    fn parse(attr: Attribute, s: &str) -> Result<Self, NodeError> {
        match s {
            "fractalNoise" => Ok(NoiseType::FractalNoise),
            "turbulence" => Ok(NoiseType::Turbulence),
            _ => Err(NodeError::parse_error(
                attr,
                ParseError::new("invalid value"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turbulence_rng() {
        let mut r = 1;
        r = setup_seed(r);

        for _ in 0..10_000 {
            r = random(r);
        }

        assert_eq!(r, 1043618065);
    }
}
