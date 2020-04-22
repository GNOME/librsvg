//! The `image` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::{ClipMode, DrawingCtx, ViewParams};
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::*;
use crate::length::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::ParseValue;
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::rect::Rect;
use crate::url_resolver::Href;
use crate::viewbox::ViewBox;

#[derive(Default)]
pub struct Image {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: Length<Horizontal>,
    height: Length<Vertical>,
    aspect: AspectRatio,
    href: Option<Href>,
}

impl SetAttributes for Image {
    fn set_attributes(&mut self, pbag: &PropertyBag<'_>) -> ElementResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => {
                    self.width = attr.parse_and_validate(value, Length::check_nonnegative)?
                }
                expanded_name!("", "height") => {
                    self.height = attr.parse_and_validate(value, Length::check_nonnegative)?
                }
                expanded_name!("", "preserveAspectRatio") => self.aspect = attr.parse(value)?,

                // "path" is used by some older Adobe Illustrator versions
                expanded_name!(xlink "href") | expanded_name!("", "path") => {
                    let href = Href::parse(value)
                        .map_err(|_| ValueErrorKind::parse_error("could not parse href"))
                        .attribute(attr)?;

                    self.href = Some(href);
                }

                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Image {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        let params = draw_ctx.get_view_params();

        let rect = self.get_rect(values, &params);

        if rect.is_empty() || self.href.is_none() {
            return Ok(draw_ctx.empty_bbox());
        }

        let href = self.href.as_ref().unwrap();
        let url = match *href {
            Href::PlainUrl(ref url) => url,
            Href::WithFragment(_) => {
                rsvg_log!(
                    "not rendering {} because its xlink:href cannot contain a fragment identifier",
                    node
                );

                return Ok(draw_ctx.empty_bbox());
            }
        };

        draw_ctx.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            let surface = match an.lookup_image(url) {
                Ok(surf) => surf,
                Err(e) => {
                    rsvg_log!("could not load image \"{}\": {}", url, e);
                    return Ok(dc.empty_bbox());
                }
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

            dc.with_saved_cr(&mut |dc| {
                let image_width = f64::from(image_width);
                let image_height = f64::from(image_height);
                let vbox = ViewBox(Rect::from_size(image_width, image_height));

                if let Some(_params) =
                    dc.push_new_viewport(Some(vbox), rect, self.aspect, clip_mode)
                {
                    let cr = dc.get_cairo_context();

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

                // The bounding box for <image> is decided by the values of x, y, w, h
                // and not by the final computed image bounds.
                Ok(dc.empty_bbox().with_rect(rect))
            })
        })
    }
}

impl Image {
    fn get_rect(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        let x = self.x.normalize(&values, &params);
        let y = self.y.normalize(&values, &params);
        let w = self.width.normalize(&values, &params);
        let h = self.height.normalize(&values, &params);

        Rect::new(x, y, x + w, y + h)
    }
}
