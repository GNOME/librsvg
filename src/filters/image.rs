use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::aspect_ratio::AspectRatio;
use crate::attributes::Attributes;
use crate::document::{AcquiredNodes, Fragment};
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::href::{is_href, set_href};
use crate::node::{CascadedValues, Node};
use crate::parsers::ParseValue;
use crate::rect::Rect;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::viewbox::ViewBox;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Primitive};

/// The `feImage` filter primitive.
pub struct FeImage {
    base: Primitive,
    aspect: AspectRatio,
    href: Option<String>,
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
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
        bounds: Rect,
        fragment: &Fragment,
    ) -> Result<SharedImageSurface, FilterError> {
        let acquired_drawable = acquired_nodes
            .acquire(fragment)
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

        Ok(surface)
    }

    /// Renders the filter if the source is an external image.
    fn render_external_image(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        _draw_ctx: &DrawingCtx,
        bounds: Rect,
        unclipped_bounds: &Rect,
        url: &str,
    ) -> Result<SharedImageSurface, FilterError> {
        // FIXME: translate the error better here
        let image = acquired_nodes
            .lookup_image(url)
            .map_err(|_| FilterError::InvalidInput)?;

        let rect = self.aspect.compute(
            &ViewBox::from(Rect::from_size(
                f64::from(image.width()),
                f64::from(image.height()),
            )),
            &unclipped_bounds,
        );

        let surface = ctx
            .source_graphic()
            .paint_image(bounds, &image, Some(rect))?;

        Ok(surface)
    }
}

impl SetAttributes for FeImage {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => self.aspect = attr.parse(value)?,

                // "path" is used by some older Adobe Illustrator versions
                ref a if is_href(a) || *a == expanded_name!("", "path") => {
                    set_href(a, &mut self.href, value.to_string());
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
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let bounds_builder = self.base.get_bounds(ctx, node.parent().as_ref())?;
        let bounds = bounds_builder.into_rect(draw_ctx);

        let href = self.href.as_ref().ok_or(FilterError::InvalidInput)?;

        let surface = if let Ok(fragment) = Fragment::parse(href) {
            // if there is a fragment, render as a node
            self.render_node(ctx, acquired_nodes, draw_ctx, bounds, &fragment)
        } else {
            // if there is no fragment, render as an image
            let unclipped_bounds = bounds_builder.into_rect_without_clipping(draw_ctx);
            self.render_external_image(
                ctx,
                acquired_nodes,
                draw_ctx,
                bounds,
                &unclipped_bounds,
                href,
            )
        }?;

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface,
                bounds: bounds.into(),
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}
