use cairo;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{CompositingAffines, DrawingCtx};
use crate::error::RenderingError;
use crate::length::{LengthHorizontal, LengthVertical};
use crate::node::{CascadedValues, NodeDraw, NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::property_defs::Opacity;
use crate::surface_utils::{shared_surface::SharedImageSurface, shared_surface::SurfaceType};

coord_units!(MaskUnits, CoordUnits::ObjectBoundingBox);
coord_units!(MaskContentUnits, CoordUnits::UserSpaceOnUse);

pub struct NodeMask {
    x: LengthHorizontal,
    y: LengthVertical,
    width: LengthHorizontal,
    height: LengthVertical,

    units: MaskUnits,
    content_units: MaskContentUnits,
}

impl Default for NodeMask {
    fn default() -> NodeMask {
        NodeMask {
            // these values are per the spec
            x: LengthHorizontal::parse_str("-10%").unwrap(),
            y: LengthVertical::parse_str("-10%").unwrap(),
            width: LengthHorizontal::parse_str("120%").unwrap(),
            height: LengthVertical::parse_str("120%").unwrap(),

            units: MaskUnits::default(),
            content_units: MaskContentUnits::default(),
        }
    }
}

impl NodeMask {
    pub fn generate_cairo_mask(
        &self,
        mask_node: &RsvgNode,
        affines: &CompositingAffines,
        draw_ctx: &mut DrawingCtx,
        bbox: &BoundingBox,
    ) -> Result<Option<cairo::ImageSurface>, RenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(None);
        }

        let bbox_rect = bbox.rect.as_ref().unwrap();

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
            mask_cr.set_matrix(affines.for_temporary_surface);
            mask_cr.transform(mask_node.borrow().get_transform());

            draw_ctx.push_cairo_context(mask_cr);

            if mask_units == CoordUnits::ObjectBoundingBox {
                draw_ctx.clip(
                    x * bbox_rect.width + bbox_rect.x,
                    y * bbox_rect.height + bbox_rect.y,
                    w * bbox_rect.width,
                    h * bbox_rect.height,
                );
            } else {
                draw_ctx.clip(x, y, w, h);
            }

            {
                let _params = if content_units == CoordUnits::ObjectBoundingBox {
                    let bbtransform = cairo::Matrix::new(
                        bbox_rect.width,
                        0.0,
                        0.0,
                        bbox_rect.height,
                        bbox_rect.x,
                        bbox_rect.y,
                    );

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

impl NodeTrait for NodeMask {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "width") => {
                    self.width =
                        attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?
                }
                expanded_name!(svg "height") => {
                    self.height =
                        attr.parse_and_validate(value, LengthVertical::check_nonnegative)?
                }
                expanded_name!(svg "maskUnits") => self.units = attr.parse(value)?,
                expanded_name!(svg "maskContentUnits") => self.content_units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}
