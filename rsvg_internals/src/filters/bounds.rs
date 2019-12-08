//! Filter primitive subregion computation.
use cairo;

use crate::bbox::BoundingBox;
use crate::drawing_ctx::DrawingCtx;
use crate::length::*;
use crate::rect::IRect;

use super::context::{FilterContext, FilterInput};

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
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    width: Option<Length<Horizontal>>,
    height: Option<Length<Vertical>>,
}

impl<'a> BoundsBuilder<'a> {
    /// Constructs a new `BoundsBuilder`.
    #[inline]
    pub fn new(
        ctx: &'a FilterContext,
        x: Option<Length<Horizontal>>,
        y: Option<Length<Vertical>>,
        width: Option<Length<Horizontal>>,
        height: Option<Length<Vertical>>,
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
            FilterInput::PrimitiveOutput(ref output) => {
                let input_bbox =
                    BoundingBox::new(&cairo::Matrix::identity()).with_rect(output.bounds.into());
                self.bbox.insert(&input_bbox);
            }
        }

        self
    }

    /// Returns the final pixel bounds.
    #[inline]
    pub fn into_irect(self, draw_ctx: &mut DrawingCtx) -> IRect {
        let mut bbox = self.apply_properties(draw_ctx);

        let effects_region = self.ctx.effects_region();
        bbox.clip(&effects_region);

        bbox.rect.unwrap().into()
    }

    /// Returns the final pixel bounds without clipping to the filter effects region.
    ///
    /// Used by feImage.
    #[inline]
    pub fn into_irect_without_clipping(self, draw_ctx: &mut DrawingCtx) -> IRect {
        self.apply_properties(draw_ctx).rect.unwrap().into()
    }

    /// Applies the filter primitive properties.
    fn apply_properties(mut self, draw_ctx: &mut DrawingCtx) -> BoundingBox {
        if self.bbox.rect.is_none() || self.standard_input_was_referenced {
            // The default value is the filter effects region.
            let effects_region = self.ctx.effects_region();

            // Clear out the rect.
            self.bbox.clear();

            // Convert into the paffine coordinate system.
            self.bbox.insert(&effects_region);
        }

        // If any of the properties were specified, we need to respect them.
        if self.x.is_some() || self.y.is_some() || self.width.is_some() || self.height.is_some() {
            let params = self.ctx.get_view_params(draw_ctx);
            let values = self.ctx.get_computed_values_from_node_being_filtered();

            // These replacements are correct only because self.bbox is used with the paffine
            // matrix.
            let rect = self.bbox.rect.as_mut().unwrap();

            if let Some(x) = self.x {
                rect.x0 = x.normalize(values, &params);
            }
            if let Some(y) = self.y {
                rect.y0 = y.normalize(values, &params);
            }
            if let Some(width) = self.width {
                rect.x1 = rect.x0 + width.normalize(values, &params);
            }
            if let Some(height) = self.height {
                rect.y1 = rect.y0 + height.normalize(values, &params);
            }
        }

        // Convert into the surface coordinate system.
        let mut bbox = BoundingBox::new(&cairo::Matrix::identity());
        bbox.insert(&self.bbox);
        bbox
    }
}
