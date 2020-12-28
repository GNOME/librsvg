//! Filter primitive subregion computation.
use crate::drawing_ctx::DrawingCtx;
use crate::length::*;
use crate::rect::{IRect, Rect};
use crate::transform::Transform;

use super::context::{FilterContext, FilterInput};

/// A helper type for filter primitive subregion computation.
#[derive(Clone, Copy)]
pub struct BoundsBuilder<'a> {
    /// The filter context.
    ctx: &'a FilterContext,

    /// The transform to use when generating the rect
    transform: Transform,

    /// The inverse transform used when adding rects
    inverse: Transform,

    /// The current bounding rectangle.
    rect: Option<Rect>,

    /// Whether one of the input nodes is standard input.
    standard_input_was_referenced: bool,

    /// Filter primitive properties.
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    width: Option<ULength<Horizontal>>,
    height: Option<ULength<Vertical>>,
}

impl<'a> BoundsBuilder<'a> {
    /// Constructs a new `BoundsBuilder`.
    #[inline]
    pub fn new(
        ctx: &'a FilterContext,
        x: Option<Length<Horizontal>>,
        y: Option<Length<Vertical>>,
        width: Option<ULength<Horizontal>>,
        height: Option<ULength<Vertical>>,
    ) -> Self {
        // FIXME: we panic if paffine is not invertible... do we need to check here?
        Self {
            ctx,
            transform: ctx.paffine(),
            inverse: ctx.paffine().invert().unwrap(),
            rect: None,
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
        // regardless of other referenced inputs. This means we can skip computing the bounds.
        if self.standard_input_was_referenced {
            return self;
        }

        match *input {
            FilterInput::StandardInput(_) => {
                self.standard_input_was_referenced = true;
            }
            FilterInput::PrimitiveOutput(ref output) => {
                let input_rect = self.inverse.transform_rect(&Rect::from(output.bounds));
                self.rect = Some(self.rect.map_or(input_rect, |r| input_rect.union(&r)));
            }
        }

        self
    }

    /// Returns the final exact bounds.
    pub fn into_rect(self, draw_ctx: &mut DrawingCtx) -> Rect {
        self.into_rect_without_clipping(draw_ctx)
            .intersection(&self.ctx.effects_region())
            .unwrap_or_else(Rect::default)
    }

    /// Returns the final pixel bounds.
    pub fn into_irect(self, draw_ctx: &mut DrawingCtx) -> IRect {
        self.into_rect(draw_ctx).into()
    }

    /// Returns the final exact bounds without clipping to the filter effects region.
    pub fn into_rect_without_clipping(self, draw_ctx: &mut DrawingCtx) -> Rect {
        // The default value is the filter effects region converted into
        // the paffine coordinate system.
        let mut rect = match self.rect {
            Some(r) if !self.standard_input_was_referenced => r,
            _ => self.inverse.transform_rect(&self.ctx.effects_region()),
        };

        // If any of the properties were specified, we need to respect them.
        // These replacements are possible because of the paffince coordinate system.
        if self.x.is_some() || self.y.is_some() || self.width.is_some() || self.height.is_some() {
            let params = draw_ctx.push_coord_units(self.ctx.primitive_units());
            let values = self.ctx.get_computed_values_from_node_being_filtered();

            if let Some(x) = self.x {
                let w = rect.width();
                rect.x0 = x.normalize(values, &params);
                rect.x1 = rect.x0 + w;
            }
            if let Some(y) = self.y {
                let h = rect.height();
                rect.y0 = y.normalize(values, &params);
                rect.y1 = rect.y0 + h;
            }
            if let Some(width) = self.width {
                rect.x1 = rect.x0 + width.normalize(values, &params);
            }
            if let Some(height) = self.height {
                rect.y1 = rect.y0 + height.normalize(values, &params);
            }
        }

        // Convert into the surface coordinate system.
        self.transform.transform_rect(&rect)
    }
}
