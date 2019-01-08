use std::cell::{Cell, RefCell};

use cairo::{self, ImageSurface, MatrixTrait, PatternTrait};

use aspect_ratio::AspectRatio;
use attributes::Attribute;
use defs::{Fragment, Href};
use drawing_ctx::DrawingCtx;
use error::{NodeError, RenderingError};
use handle::{self, LoadOptions};
use node::{CascadedValues, NodeResult, NodeTrait, RsvgNode};
use parsers::{ParseError, ParseValue};
use property_bag::PropertyBag;
use rect::IRect;
use surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

use super::bounds::BoundsBuilder;
use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{Filter, FilterError, Primitive};

/// The `feImage` filter primitive.
pub struct Image {
    base: Primitive,
    aspect: Cell<AspectRatio>,
    href: RefCell<Option<Href>>,
    load_options: RefCell<Option<LoadOptions>>,
}

impl Image {
    /// Constructs a new `Image` with empty properties.
    #[inline]
    pub fn new() -> Image {
        Image {
            base: Primitive::new::<Self>(),
            aspect: Cell::new(AspectRatio::default()),
            href: RefCell::new(None),
            load_options: RefCell::new(None),
        }
    }

    fn set_href(&self, attr: Attribute, href_str: &str) -> NodeResult {
        let href = Href::parse(href_str)
            .map_err(|_| NodeError::parse_error(attr, ParseError::new("could not parse href")))?;

        *self.href.borrow_mut() = Some(href);

        Ok(())
    }

    /// Renders the filter if the source is an existing node.
    fn render_node(
        &self,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
        bounds: IRect,
        fragment: &Fragment,
    ) -> Result<ImageSurface, FilterError> {
        let acquired_drawable = draw_ctx
            .get_acquired_node(fragment)
            .ok_or(FilterError::InvalidInput)?;
        let drawable = acquired_drawable.get();

        let surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        draw_ctx.get_cairo_context().set_matrix(ctx.paffine());

        let node_being_filtered_values = ctx.get_computed_values_from_node_being_filtered();

        let cascaded = CascadedValues::new_from_values(&drawable, node_being_filtered_values);

        draw_ctx
            .draw_node_on_surface(
                &drawable,
                &cascaded,
                &surface,
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
        cr.rectangle(
            f64::from(bounds.x0),
            f64::from(bounds.y0),
            f64::from(bounds.x1 - bounds.x0),
            f64::from(bounds.y1 - bounds.y0),
        );
        cr.clip();
        cr.set_source_surface(&surface, 0f64, 0f64);
        cr.paint();

        Ok(output_surface)
    }

    /// Renders the filter if the source is an external image.
    fn render_external_image(
        &self,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
        bounds_builder: BoundsBuilder<'_>,
        href: &Href,
    ) -> Result<ImageSurface, FilterError> {
        let url = if let Href::PlainUri(ref url) = *href {
            url
        } else {
            unreachable!();
        };

        let load_options_ref = self.load_options.borrow();

        // FIXME: translate the error better here
        let surface = handle::load_image_to_surface(load_options_ref.as_ref().unwrap(), &url)
            .map_err(|_| FilterError::InvalidInput)?;

        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        // TODO: this goes through a f64->i32->f64 conversion.
        let render_bounds = bounds_builder.into_irect_without_clipping(draw_ctx);
        let aspect = self.aspect.get();
        let (x, y, w, h) = aspect.compute(
            f64::from(surface.get_width()),
            f64::from(surface.get_height()),
            f64::from(render_bounds.x0),
            f64::from(render_bounds.y0),
            f64::from(render_bounds.x1 - render_bounds.x0),
            f64::from(render_bounds.y1 - render_bounds.y0),
        );

        if w != 0f64 && h != 0f64 {
            let ptn = cairo::SurfacePattern::create(&surface);
            let mut matrix = cairo::Matrix::new(
                w / f64::from(surface.get_width()),
                0f64,
                0f64,
                h / f64::from(surface.get_height()),
                x,
                y,
            );
            matrix.invert();
            ptn.set_matrix(matrix);

            let bounds = bounds_builder.into_irect(draw_ctx);
            let cr = cairo::Context::new(&output_surface);
            cr.rectangle(
                f64::from(bounds.x0),
                f64::from(bounds.y0),
                f64::from(bounds.x1 - bounds.x0),
                f64::from(bounds.y1 - bounds.y0),
            );
            cr.clip();
            cr.set_source(&cairo::Pattern::SurfacePattern(ptn));
            cr.paint();
        }

        Ok(output_surface)
    }
}

impl NodeTrait for Image {
    fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(node, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::PreserveAspectRatio => self.aspect.set(attr.parse(value, ())?),

                // "path" is used by some older Adobe Illustrator versions
                Attribute::XlinkHref | Attribute::Path => self.set_href(attr, value)?,

                _ => (),
            }
        }

        Ok(())
    }

    fn resolve_resources(&self, load_options: &LoadOptions) -> NodeResult {
        *self.load_options.borrow_mut() = Some(load_options.clone());

        Ok(())
    }
}

impl Filter for Image {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let bounds_builder = self.base.get_bounds(ctx);
        let bounds = bounds_builder.into_irect(draw_ctx);

        let href_borrow = self.href.borrow();
        let href_opt = href_borrow.as_ref();

        if let Some(href) = href_opt {
            let output_surface = match *href {
                Href::PlainUri(_) => {
                    self.render_external_image(ctx, draw_ctx, bounds_builder, href)?
                }
                Href::WithFragment(ref frag) => self.render_node(ctx, draw_ctx, bounds, frag)?,
            };

            Ok(FilterResult {
                name: self.base.result.borrow().clone(),
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
