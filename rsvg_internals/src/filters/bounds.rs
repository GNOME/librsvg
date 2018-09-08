//! Filter primitive subregion computation.
use cairo::{self, MatrixTrait};

use bbox::BoundingBox;
use drawing_ctx::DrawingCtx;
use length::Length;

use super::context::{FilterContext, FilterInput, FilterOutput, IRect};

/// A helper type for filter primitive subregion computation.
#[derive(Clone, Copy)]
pub struct BoundsBuilder<'a> {
    /// The filter context.
    ctx: &'a FilterContext,

    /// The current bounding box.
    bbox: BoundingBox,

    /// Whether one of the input nodes is standard input.
    standard_input_was_referenced: bool,

    /// Filter primitive properties.
    x: Option<Length>,
    y: Option<Length>,
    width: Option<Length>,
    height: Option<Length>,
}

impl<'a> BoundsBuilder<'a> {
    /// Constructs a new `BoundsBuilder`.
    #[inline]
    pub fn new(
        ctx: &'a FilterContext,
        x: Option<Length>,
        y: Option<Length>,
        width: Option<Length>,
        height: Option<Length>,
    ) -> Self {
        Self {
            ctx,
            // The matrix is paffine because we're using that fact in apply_properties().
            bbox: BoundingBox::new(&ctx.paffine()),
            standard_input_was_referenced: false,
            x,
            y,
            width,
            height,
        }
    }

    /// Adds a filter primitive input to the bounding box.
    #[inline]
    pub fn add_input(mut self, input: &FilterInput) -> Self {
        // If a standard input was referenced, the default value is the filter effects region
        // regardless of other referenced inputs. This means we can skip computing the bounding
        // box.
        if self.standard_input_was_referenced {
            return self;
        }

        match *input {
            FilterInput::StandardInput(_) => {
                self.standard_input_was_referenced = true;
            }
            FilterInput::PrimitiveOutput(FilterOutput {
                bounds: IRect { x0, y0, x1, y1 },
                ..
            }) => {
                let rect = cairo::Rectangle {
                    x: f64::from(x0),
                    y: f64::from(y0),
                    width: f64::from(x1 - x0),
                    height: f64::from(y1 - y0),
                };

                let input_bbox = BoundingBox::new(&cairo::Matrix::identity()).with_rect(Some(rect));
                self.bbox.insert(&input_bbox);
            }
        }

        self
    }

    /// Returns the final pixel bounds.
    #[inline]
    pub fn into_irect(self, draw_ctx: &mut DrawingCtx<'_>) -> IRect {
        let mut bbox = self.apply_properties(draw_ctx);

        let effects_region = self.ctx.effects_region();
        bbox.clip(&effects_region);

        bbox.rect.unwrap().into()
    }

    /// Returns the final pixel bounds without clipping to the filter effects region.
    ///
    /// Used by feImage.
    #[inline]
    pub fn into_irect_without_clipping(self, draw_ctx: &mut DrawingCtx<'_>) -> IRect {
        self.apply_properties(draw_ctx).rect.unwrap().into()
    }

    /// Applies the filter primitive properties.
    fn apply_properties(mut self, draw_ctx: &mut DrawingCtx<'_>) -> BoundingBox {
        if self.bbox.rect.is_none() || self.standard_input_was_referenced {
            // The default value is the filter effects region.
            let effects_region = self.ctx.effects_region();

            // Clear out the rect.
            self.bbox = self.bbox.with_rect(None);
            // Convert into the paffine coordinate system.
            self.bbox.insert(&effects_region);
        }

        // If any of the properties were specified, we need to respect them.
        if self.x.is_some() || self.y.is_some() || self.width.is_some() || self.height.is_some() {
            self.ctx.with_primitive_units(draw_ctx, |normalize| {
                // These replacements are correct only because self.bbox is used with the paffine
                // matrix.
                let rect = self.bbox.rect.as_mut().unwrap();

                if let Some(x) = self.x {
                    rect.x = normalize(&x);
                }
                if let Some(y) = self.y {
                    rect.y = normalize(&y);
                }
                if let Some(width) = self.width {
                    rect.width = normalize(&width);
                }
                if let Some(height) = self.height {
                    rect.height = normalize(&height);
                }
            });
        }

        // Convert into the surface coordinate system.
        let mut bbox = BoundingBox::new(&cairo::Matrix::identity());
        bbox.insert(&self.bbox);
        bbox
    }
}
