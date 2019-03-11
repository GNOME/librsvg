use cairo::{self, MatrixTrait};
use std::cell::Cell;

use crate::attributes::Attribute;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::error::RenderingError;
use crate::length::{LengthHorizontal, LengthVertical};
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseValue};
use crate::properties::Opacity;
use crate::property_bag::PropertyBag;
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
    x: Cell<LengthHorizontal>,
    y: Cell<LengthVertical>,
    width: Cell<LengthHorizontal>,
    height: Cell<LengthVertical>,

    units: Cell<MaskUnits>,
    content_units: Cell<MaskContentUnits>,
}

impl NodeMask {
    pub fn new() -> NodeMask {
        NodeMask {
            // these values are per the spec
            x: Cell::new(LengthHorizontal::parse_str("-10%").unwrap()),
            y: Cell::new(LengthVertical::parse_str("-10%").unwrap()),

            width: Cell::new(LengthHorizontal::parse_str("120%").unwrap()),
            height: Cell::new(LengthVertical::parse_str("120%").unwrap()),

            units: Cell::new(MaskUnits::default()),
            content_units: Cell::new(MaskContentUnits::default()),
        }
    }

    pub fn generate_cairo_mask(
        &self,
        node: &RsvgNode,
        affine_before_mask: &cairo::Matrix,
        draw_ctx: &mut DrawingCtx,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        let surface = draw_ctx.create_surface_for_toplevel_viewport()?;

        let mask_units = CoordUnits::from(self.units.get());
        let content_units = CoordUnits::from(self.content_units.get());

        let (x, y, w, h) = {
            let params = if mask_units == CoordUnits::ObjectBoundingBox {
                draw_ctx.push_view_box(1.0, 1.0)
            } else {
                draw_ctx.get_view_params()
            };

            let x = self.x.get().normalize(&values, &params);
            let y = self.y.get().normalize(&values, &params);
            let w = self.width.get().normalize(&values, &params);
            let h = self.height.get().normalize(&values, &params);

            (x, y, w, h)
        };

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let bbox_rect = {
                if let Some(ref rect) = bbox.rect {
                    *rect
                } else {
                    // The node being masked is empty / doesn't have a
                    // bounding box, so there's nothing to mask!
                    return Ok(());
                }
            };

            let save_cr = draw_ctx.get_cairo_context();

            let mask_cr = cairo::Context::new(&surface);
            mask_cr.set_matrix(*affine_before_mask);
            mask_cr.transform(node.get_transform());

            draw_ctx.set_cairo_context(&mask_cr);

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

                    mask_cr.transform(bbtransform);

                    draw_ctx.push_view_box(1.0, 1.0)
                } else {
                    draw_ctx.get_view_params()
                };

                let res = draw_ctx.with_discrete_layer(node, values, false, &mut |dc| {
                    node.draw_children(&cascaded, dc, false)
                });

                draw_ctx.set_cairo_context(&save_cr);

                res
            }
        }?;

        let Opacity(opacity) = values.opacity;
        let mask_surface = compute_luminance_to_alpha(surface, opacity)?;
        draw_ctx.mask_surface(&mask_surface);

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
    surface: cairo::ImageSurface,
    opacity: UnitInterval,
) -> Result<cairo::ImageSurface, cairo::Status> {
    let surface = SharedImageSurface::new(surface, SurfaceType::SRgb)?;

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

        for (x, y, pixel) in Pixels::new(&surface, bounds) {
            output_data.set_pixel(output_stride, pixel.to_mask(opacity), x, y);
        }
    }

    Ok(output)
}

impl NodeTrait for NodeMask {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(attr.parse(value)?),
                Attribute::Y => self.y.set(attr.parse(value)?),
                Attribute::Width => self
                    .width
                    .set(attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?),
                Attribute::Height => self
                    .height
                    .set(attr.parse_and_validate(value, LengthVertical::check_nonnegative)?),
                Attribute::MaskUnits => self.units.set(attr.parse(value)?),
                Attribute::MaskContentUnits => self.content_units.set(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }
}
