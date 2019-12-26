//! The `mask` element.

use cairo;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::error::RenderingError;
use crate::length::*;
use crate::node::{CascadedValues, NodeDraw, NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::property_defs::Opacity;
use crate::rect::Rect;
use crate::surface_utils::{shared_surface::SharedImageSurface, shared_surface::SurfaceType};

coord_units!(MaskUnits, CoordUnits::ObjectBoundingBox);
coord_units!(MaskContentUnits, CoordUnits::UserSpaceOnUse);

pub struct Mask {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: Length<Horizontal>,
    height: Length<Vertical>,

    units: MaskUnits,
    content_units: MaskContentUnits,
}

impl Default for Mask {
    fn default() -> Mask {
        Mask {
            // these values are per the spec
            x: Length::<Horizontal>::parse_str("-10%").unwrap(),
            y: Length::<Vertical>::parse_str("-10%").unwrap(),
            width: Length::<Horizontal>::parse_str("120%").unwrap(),
            height: Length::<Vertical>::parse_str("120%").unwrap(),

            units: MaskUnits::default(),
            content_units: MaskContentUnits::default(),
        }
    }
}

impl Mask {
    pub fn generate_cairo_mask(
        &self,
        mask_node: &RsvgNode,
        affine: cairo::Matrix,
        draw_ctx: &mut DrawingCtx,
        bbox: &BoundingBox,
    ) -> Result<Option<cairo::ImageSurface>, RenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(None);
        }

        let bbox_rect = bbox.rect.as_ref().unwrap();
        let (bb_x, bb_y) = (bbox_rect.x0, bbox_rect.y0);
        let (bb_w, bb_h) = bbox_rect.size();

        let cascaded = CascadedValues::new_from_node(mask_node);
        let values = cascaded.get();

        let mask_content_surface = draw_ctx.create_surface_for_toplevel_viewport()?;

        let mask_units = CoordUnits::from(self.units);
        let content_units = CoordUnits::from(self.content_units);

        let (x, y, w, h) = {
            let params = if mask_units == CoordUnits::ObjectBoundingBox {
                draw_ctx.push_view_box(1.0, 1.0)
            } else {
                draw_ctx.get_view_params()
            };

            let x = self.x.normalize(&values, &params);
            let y = self.y.normalize(&values, &params);
            let w = self.width.normalize(&values, &params);
            let h = self.height.normalize(&values, &params);

            (x, y, w, h)
        };

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let mask_cr = cairo::Context::new(&mask_content_surface);
            mask_cr.set_matrix(affine);

            draw_ctx.push_cairo_context(mask_cr);

            let (x, y, w, h) = if mask_units == CoordUnits::ObjectBoundingBox {
                (x * bb_w + bb_x, y * bb_h + bb_y, w * bb_w, h * bb_h)
            } else {
                (x, y, w, h)
            };

            draw_ctx.clip(Rect::new(x, y, x + w, y + h));

            {
                let _params = if content_units == CoordUnits::ObjectBoundingBox {
                    let bbtransform = cairo::Matrix::new(bb_w, 0.0, 0.0, bb_h, bb_x, bb_y);

                    draw_ctx.get_cairo_context().transform(bbtransform);

                    draw_ctx.push_view_box(1.0, 1.0)
                } else {
                    draw_ctx.get_view_params()
                };

                let res = draw_ctx.with_discrete_layer(mask_node, values, false, &mut |dc| {
                    mask_node.draw_children(&cascaded, dc, false)
                });

                draw_ctx.pop_cairo_context();

                res
            }
        }?;

        let Opacity(opacity) = values.opacity;

        let mask = SharedImageSurface::new(mask_content_surface, SurfaceType::SRgb)?
            .to_mask(u8::from(opacity))?
            .into_image_surface()?;

        Ok(Some(mask))
    }
}

impl NodeTrait for Mask {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "width") => {
                    self.width =
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?
                }
                expanded_name!(svg "height") => {
                    self.height =
                        attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?
                }
                expanded_name!(svg "maskUnits") => self.units = attr.parse(value)?,
                expanded_name!(svg "maskContentUnits") => self.content_units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}
