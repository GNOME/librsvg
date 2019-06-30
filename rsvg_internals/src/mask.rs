use cairo::{self, MatrixTrait};
use markup5ever::local_name;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{CompositingAffines, DrawingCtx};
use crate::error::RenderingError;
use crate::length::{LengthHorizontal, LengthVertical};
use crate::node::{CascadedValues, NodeDraw, NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::property_defs::Opacity;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
    ImageSurfaceDataExt,
};
use crate::unit_interval::UnitInterval;

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
    ) -> Result<(), RenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(());
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

        let mask_content_surface =
            SharedImageSurface::new(mask_content_surface, SurfaceType::SRgb)?;

        let mask_surface = compute_luminance_to_alpha(&mask_content_surface, opacity)?;

        let cr = draw_ctx.get_cairo_context();
        cr.set_matrix(affines.compositing);

        cr.mask_surface(&mask_surface, 0.0, 0.0);

        Ok(())
    }
}

// Returns a surface whose alpha channel for each pixel is equal to the
// luminance of that pixel's unpremultiplied RGB values.  The resulting
// surface's RGB values are not meanignful; only the alpha channel has
// useful luminance data.
//
// This is to get a mask suitable for use with cairo_mask_surface().
fn compute_luminance_to_alpha(
    surface: &SharedImageSurface,
    opacity: UnitInterval,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let width = surface.width();
    let height = surface.height();

    let bounds = IRect {
        x0: 0,
        y0: 0,
        x1: width,
        y1: height,
    };

    let opacity = u8::from(opacity);
    let mut output = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let output_stride = output.get_stride() as usize;

    {
        let mut output_data = output.get_data().unwrap();

        for (x, y, pixel) in Pixels::new(surface, bounds) {
            output_data.set_pixel(output_stride, pixel.to_mask(opacity), x, y);
        }
    }

    Ok(output)
}

impl NodeTrait for NodeMask {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("x") => self.x = attr.parse(value)?,
                local_name!("y") => self.y = attr.parse(value)?,
                local_name!("width") => {
                    self.width =
                        attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?
                }
                local_name!("height") => {
                    self.height =
                        attr.parse_and_validate(value, LengthVertical::check_nonnegative)?
                }
                local_name!("maskUnits") => self.units = attr.parse(value)?,
                local_name!("maskContentUnits") => self.content_units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}
