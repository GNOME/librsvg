use std::cell::Cell;
use std::cmp::min;
use std::f64;

use cairo::MatrixTrait;
use nalgebra::{DMatrix, Dynamic, MatrixVec};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::NodeError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers;
use property_bag::PropertyBag;
use surface_utils::{
    shared_surface::{BlurDirection, Horizontal, SharedImageSurface, Vertical},
    EdgeMode,
};

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::{Filter, FilterError, PrimitiveWithInput};

/// The maximum gaussian blur kernel size.
///
/// The value of 500 is used in webkit.
const MAXIMUM_KERNEL_SIZE: usize = 500;

/// The `feGaussianBlur` filter primitive.
pub struct GaussianBlur {
    base: PrimitiveWithInput,
    std_deviation: Cell<(f64, f64)>,
}

impl GaussianBlur {
    /// Constructs a new `GaussianBlur` with empty properties.
    #[inline]
    pub fn new() -> GaussianBlur {
        GaussianBlur {
            base: PrimitiveWithInput::new::<Self>(),
            std_deviation: Cell::new((0.0, 0.0)),
        }
    }
}

impl NodeTrait for GaussianBlur {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::StdDeviation => self.std_deviation.set(
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
                _ => (),
            }
        }

        Ok(())
    }
}

/// Computes a gaussian kernel line for the given standard deviation.
fn gaussian_kernel(std_deviation: f64) -> Vec<f64> {
    assert!(std_deviation > 0.0);

    // Make sure there aren't any infinities.
    let maximal_deviation = (MAXIMUM_KERNEL_SIZE / 2) as f64 / 3.0;

    // Values further away than std_deviation * 3 are too small to contribute anything meaningful.
    let radius = ((std_deviation.min(maximal_deviation) * 3.0) + 0.5) as usize;
    // Clamp the radius rather than diameter because `MAXIMUM_KERNEL_SIZE` might be even and we
    // want an odd-sized kernel.
    let radius = min(radius, (MAXIMUM_KERNEL_SIZE - 1) / 2);
    let diameter = radius * 2 + 1;

    let mut kernel = Vec::with_capacity(diameter);

    let gauss_point = |x: f64| (-x.powi(2) / (2.0 * std_deviation.powi(2))).exp();

    // Fill the matrix by doing numerical integration approximation from -2*std_dev to 2*std_dev,
    // sampling 50 points per pixel. We do the bottom half, mirror it to the top half, then compute
    // the center point. Otherwise asymmetric quantization errors will occur. The formula to
    // integrate is e^-(x^2/2s^2).
    for i in 0..diameter / 2 {
        let base_x = (diameter / 2 + 1 - i) as f64 - 0.5;

        let mut sum = 0.0;
        for j in 1..=50 {
            let r = base_x + 0.02 * f64::from(j);
            sum += gauss_point(r);
        }

        kernel.push(sum / 50.0);
    }

    // We'll compute the middle point later.
    kernel.push(0.0);

    // Mirror the bottom half to the top half.
    for i in 0..diameter / 2 {
        let x = kernel[diameter / 2 - 1 - i];
        kernel.push(x);
    }

    // Find center val -- calculate an odd number of quanta to make it symmetric, even if the
    // center point is weighted slightly higher than others.
    let mut sum = 0.0;
    for j in 0..=50 {
        let r = -0.5 + 0.02 * f64::from(j);
        sum += gauss_point(r);
    }
    kernel[diameter / 2] = sum / 51.0;

    // Normalize the distribution by scaling the total sum to 1.
    let sum = kernel.iter().sum::<f64>();
    kernel.iter_mut().for_each(|x| *x /= sum);

    kernel
}

/// Returns a size of the box blur kernel to approximate the gaussian blur.
fn box_blur_kernel_size(std_deviation: f64) -> usize {
    let d = (std_deviation * 3.0 * (2.0 * f64::consts::PI).sqrt() / 4.0 + 0.5).floor();
    let d = d.min(MAXIMUM_KERNEL_SIZE as f64);
    d as usize
}

/// Applies three box blurs to approximate the gaussian blur.
///
/// This is intended to be used in two steps, horizontal and vertical.
fn three_box_blurs<B: BlurDirection>(
    surface: &SharedImageSurface,
    bounds: IRect,
    std_deviation: f64,
) -> Result<SharedImageSurface, FilterError> {
    let d = box_blur_kernel_size(std_deviation);
    if d == 0 {
        return Ok(surface.clone());
    }

    let surface = if d % 2 == 1 {
        // Odd kernel sizes just get three successive box blurs.
        let mut surface = surface.clone();

        for _ in 0..3 {
            surface = surface.box_blur::<B>(bounds, d, d / 2)?;
        }

        surface
    } else {
        // Even kernel sizes have a more interesting scheme.
        let surface = surface.box_blur::<B>(bounds, d, d / 2)?;
        let surface = surface.box_blur::<B>(bounds, d, d / 2 - 1)?;

        let d = d + 1;
        surface.box_blur::<B>(bounds, d, d / 2)?
    };

    Ok(surface)
}

/// Applies the gaussian blur.
///
/// This is intended to be used in two steps, horizontal and vertical.
fn gaussian_blur(
    input_surface: &SharedImageSurface,
    bounds: IRect,
    std_deviation: f64,
    vertical: bool,
) -> Result<SharedImageSurface, FilterError> {
    let kernel = gaussian_kernel(std_deviation);
    let (rows, cols) = if vertical {
        (kernel.len(), 1)
    } else {
        (1, kernel.len())
    };
    let kernel = DMatrix::from_data(MatrixVec::new(
        Dynamic::new(rows),
        Dynamic::new(cols),
        kernel,
    ));

    Ok(input_surface.convolve(
        bounds,
        ((cols / 2) as i32, (rows / 2) as i32),
        &kernel,
        EdgeMode::None,
    )?)
}

impl Filter for GaussianBlur {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);

        let (std_x, std_y) = self.std_deviation.get();
        let (std_x, std_y) = ctx.paffine().transform_distance(std_x, std_y);

        // The deviation can become negative here due to the transform.
        let std_x = std_x.abs();
        let std_y = std_y.abs();

        // Performance TODO: gaussian blur is frequently used for shadows, operating on SourceAlpha
        // (so the image is alpha-only). We can use this to not waste time processing the other
        // channels.

        // Horizontal convolution.
        let horiz_result_surface = if std_x != 0.0 {
            // The spec says for deviation >= 2.0 three box blurs can be used as an optimization.
            if std_x >= 2.0 {
                three_box_blurs::<Horizontal>(input.surface(), bounds, std_x)?
            } else {
                gaussian_blur(input.surface(), bounds, std_x, false)?
            }
        } else {
            input.surface().clone()
        };

        // Vertical convolution.
        let output_surface = if std_y != 0.0 {
            // The spec says for deviation >= 2.0 three box blurs can be used as an optimization.
            if std_y >= 2.0 {
                three_box_blurs::<Vertical>(&horiz_result_surface, bounds, std_y)?
            } else {
                gaussian_blur(&horiz_result_surface, bounds, std_y, true)?
            }
        } else {
            horiz_result_surface
        };

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
