use cairo;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::Href;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::drawing_ctx::{ClipMode, DrawingCtx};
use crate::error::{NodeError, RenderingError};
use crate::float_eq_cairo::ApproxEqCairo;
use crate::length::*;
use crate::node::*;
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;
use crate::rect::Rect;
use crate::viewbox::ViewBox;

#[derive(Default)]
pub struct Image {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    w: Length<Horizontal>,
    h: Length<Vertical>,
    aspect: AspectRatio,
    href: Option<Href>,
}

impl NodeTrait for Image {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "width") => {
                    self.w = attr.parse_and_validate(value, Length::check_nonnegative)?
                }
                expanded_name!(svg "height") => {
                    self.h = attr.parse_and_validate(value, Length::check_nonnegative)?
                }
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

    fn overflow_hidden(&self) -> bool {
        true
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        let params = draw_ctx.get_view_params();

        let x = self.x.normalize(values, &params);
        let y = self.y.normalize(values, &params);
        let w = self.w.normalize(values, &params);
        let h = self.h.normalize(values, &params);

        if w.approx_eq_cairo(0.0) || h.approx_eq_cairo(0.0) {
            return Ok(draw_ctx.empty_bbox());
        }

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            let surface = if let Some(Href::PlainUrl(ref url)) = self.href {
                dc.lookup_image(&url)?
            } else {
                return Ok(dc.empty_bbox());
            };

            let clip_mode = if !values.is_overflow() && self.aspect.is_slice() {
                Some(ClipMode::ClipToViewport)
            } else {
                None
            };

            let image_width = surface.width();
            let image_height = surface.height();
            if clipping || image_width == 0 || image_height == 0 {
                return Ok(dc.empty_bbox());
            }

            // The bounding box for <image> is decided by the values of x, y, w, h and not by
            // the final computed image bounds.
            let bbox = dc
                .empty_bbox()
                .with_rect(Some(Rect::new(x, y, x + w, y + h)));

            dc.with_saved_cr(&mut |dc| {
                let cr = dc.get_cairo_context();

                let image_width = f64::from(image_width);
                let image_height = f64::from(image_height);

                if let Some(_params) = dc.push_new_viewport(
                    Some(ViewBox::new(0.0, 0.0, image_width, image_height)),
                    Rect::new(x, y, x + w, y + h),
                    self.aspect,
                    clip_mode,
                ) {
                    // We need to set extend appropriately, so can't use cr.set_source_surface().
                    //
                    // If extend is left at its default value (None), then bilinear scaling uses
                    // transparency outside of the image producing incorrect results.
                    // For example, in svg1.1/filters-blend-01-b.svgthere's a completely
                    // opaque 100×1 image of a gradient scaled to 100×98 which ends up
                    // transparent almost everywhere without this fix (which it shouldn't).
                    let ptn = surface.to_cairo_pattern();
                    ptn.set_extend(cairo::Extend::Pad);
                    cr.set_source(&ptn);

                    // Clip is needed due to extend being set to pad.
                    cr.rectangle(0.0, 0.0, image_width, image_height);
                    cr.clip();

                    cr.paint();
                }

                Ok(bbox)
            })
        })
    }
}
