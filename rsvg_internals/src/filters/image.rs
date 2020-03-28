use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::{Fragment, Href};
use crate::aspect_ratio::AspectRatio;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, ElementTrait};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;
use crate::rect::Rect;
use crate::viewbox::ViewBox;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Primitive};

/// The `feImage` filter primitive.
pub struct FeImage {
    base: Primitive,
    aspect: AspectRatio,
    href: Option<Href>,
}

impl Default for FeImage {
    /// Constructs a new `FeImage` with empty properties.
    #[inline]
    fn default() -> FeImage {
        FeImage {
            base: Primitive::new::<Self>(),
            aspect: AspectRatio::default(),
            href: None,
        }
    }
}

impl FeImage {
    /// Renders the filter if the source is an existing node.
    fn render_node(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
        bounds: Rect,
        fragment: &Fragment,
    ) -> Result<FilterResult, FilterError> {
        let acquired_drawable = acquired_nodes
            .acquire(fragment, &[])
            .map_err(|_| FilterError::InvalidInput)?;
        let drawable = acquired_drawable.get();

        let node_being_filtered_values = ctx.get_computed_values_from_node_being_filtered();
        let cascaded = CascadedValues::new_from_values(&drawable, node_being_filtered_values);

        let image = draw_ctx.draw_node_to_surface(
            &drawable,
            acquired_nodes,
            &cascaded,
            ctx.paffine(),
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        let surface = ctx.source_graphic().paint_image(bounds, &image, None)?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface,
                bounds: bounds.into(),
            },
        })
    }

    /// Renders the filter if the source is an external image.
    fn render_external_image(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        _draw_ctx: &DrawingCtx,
        bounds: Rect,
        unclipped_bounds: &Rect,
        url: &str,
    ) -> Result<FilterResult, FilterError> {
        // FIXME: translate the error better here
        let image = acquired_nodes
            .lookup_image(url)
            .map_err(|_| FilterError::InvalidInput)?;

        // TODO: this goes through a f64->i32->f64 conversion.
        let rect = self.aspect.compute(
            &ViewBox(Rect::from_size(
                f64::from(image.width()),
                f64::from(image.height()),
            )),
            &unclipped_bounds,
        );

        let surface = ctx
            .source_graphic()
            .paint_image(bounds, &image, Some(rect))?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface,
                bounds: bounds.into(),
            },
        })
    }
}

impl ElementTrait for FeImage {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, pbag: &PropertyBag<'_>) -> ElementResult {
        self.base.set_atts(pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => self.aspect = attr.parse(value)?,

                // "path" is used by some older Adobe Illustrator versions
                expanded_name!(xlink "href") | expanded_name!("", "path") => {
                    let href = Href::parse(value)
                        .map_err(|e| ValueErrorKind::from(e))
                        .attribute(attr)?;

                    self.href = Some(href);
                }

                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeImage {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let bounds_builder = self.base.get_bounds(ctx, node.parent().as_ref())?;
        let bounds = bounds_builder.into_rect(draw_ctx);

        match self.href.as_ref() {
            Some(Href::PlainUrl(url)) => {
                let unclipped_bounds = bounds_builder.into_rect_without_clipping(draw_ctx);
                self.render_external_image(
                    ctx,
                    acquired_nodes,
                    draw_ctx,
                    bounds,
                    &unclipped_bounds,
                    url,
                )
            }
            Some(Href::WithFragment(ref frag)) => {
                self.render_node(ctx, acquired_nodes, draw_ctx, bounds, frag)
            }
            _ => Err(FilterError::InvalidInput),
        }
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}
