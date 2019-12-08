use cairo::{self, ImageSurface};
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::{Fragment, Href};
use crate::aspect_ratio::AspectRatio;
use crate::drawing_ctx::DrawingCtx;
use crate::error::{NodeError, RenderingError};
use crate::node::{CascadedValues, NodeResult, NodeTrait, RsvgNode};
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;
use crate::rect::{IRect, Rect};
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
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
        draw_ctx: &mut DrawingCtx,
        bounds: IRect,
        fragment: &Fragment,
    ) -> Result<ImageSurface, FilterError> {
        let acquired_drawable = draw_ctx
            .acquire_node(fragment, &[])
            .map_err(|_| FilterError::InvalidInput)?;
        let drawable = acquired_drawable.get();

        let surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        let node_being_filtered_values = ctx.get_computed_values_from_node_being_filtered();

        let cascaded = CascadedValues::new_from_values(&drawable, node_being_filtered_values);

        draw_ctx
            .draw_node_on_surface(
                &drawable,
                &cascaded,
                &surface,
                ctx.paffine(),
                f64::from(ctx.source_graphic().width()),
                f64::from(ctx.source_graphic().height()),
            )
            .map_err(|e| {
                if let RenderingError::Cairo(status) = e {
                    FilterError::CairoError(status)
                } else {
                    // FIXME: this is just a dummy value; we should probably have a way to indicate
                    // an error in the underlying drawing process.
                    FilterError::CairoError(cairo::Status::InvalidStatus)
                }
            })?;

        // Clip the output to bounds.
        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        let cr = cairo::Context::new(&output_surface);
        let r = cairo::Rectangle::from(bounds);
        cr.rectangle(r.x, r.y, r.width, r.height);
        cr.clip();
        cr.set_source_surface(&surface, 0f64, 0f64);
        cr.paint();

        Ok(output_surface)
    }

    /// Renders the filter if the source is an external image.
    fn render_external_image(
        &self,
        ctx: &FilterContext,
        draw_ctx: &DrawingCtx,
        bounds: IRect,
        unclipped_bounds: Rect,
        href: &Href,
    ) -> Result<ImageSurface, FilterError> {
        let surface = if let Href::PlainUrl(ref url) = *href {
            // FIXME: translate the error better here
            draw_ctx
                .lookup_image(&url)
                .map_err(|_| FilterError::InvalidInput)?
        } else {
            unreachable!();
        };

        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        // TODO: this goes through a f64->i32->f64 conversion.
        let r = self.aspect.compute(
            &ViewBox::new(Rect::from_size(
                f64::from(surface.width()),
                f64::from(surface.height()),
            )),
            unclipped_bounds,
        );

        if r.is_empty() {
            return Ok(output_surface);
        }

        let ptn = surface.to_cairo_pattern();
        let mut matrix = cairo::Matrix::new(
            r.width() / f64::from(surface.width()),
            0.0,
            0.0,
            r.height() / f64::from(surface.height()),
            r.x0,
            r.y0,
        );
        matrix.invert();
        ptn.set_matrix(matrix);

        let cr = cairo::Context::new(&output_surface);
        let r = cairo::Rectangle::from(bounds);
        cr.rectangle(r.x, r.y, r.width, r.height);
        cr.clip();
        cr.set_source(&ptn);
        cr.paint();

        Ok(output_surface)
    }
}

impl NodeTrait for FeImage {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "preserveAspectRatio") => self.aspect = attr.parse(value)?,

                // "path" is used by some older Adobe Illustrator versions
                expanded_name!(xlink "href") | expanded_name!(svg "path") => {
                    let href = Href::parse(value).map_err(|_| {
                        NodeError::parse_error(attr, "could not parse href")
                    })?;

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
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let bounds_builder = self.base.get_bounds(ctx);
        let bounds = bounds_builder.into_irect(draw_ctx);

        if let Some(href) = self.href.as_ref() {
            let output_surface = match href {
                Href::PlainUrl(_) => {
                    let unclipped_bounds = bounds_builder.into_rect_without_clipping(draw_ctx);
                    self.render_external_image(ctx, draw_ctx, bounds, unclipped_bounds, href)?
                }
                Href::WithFragment(ref frag) => self.render_node(ctx, draw_ctx, bounds, frag)?,
            };

            Ok(FilterResult {
                name: self.base.result.clone(),
                output: FilterOutput {
                    surface: SharedImageSurface::new(output_surface, SurfaceType::SRgb)?,
                    bounds,
                },
            })
        } else {
            Err(FilterError::InvalidInput)
        }
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        false
    }
}
