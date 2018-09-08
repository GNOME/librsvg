use cairo::{self, MatrixTrait};
use cairo_sys;
use glib::translate::*;
use std::cell::Cell;

use attributes::Attribute;
use coord_units::CoordUnits;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use filters::context::IRect;
use handle::RsvgHandle;
use length::{Length, LengthDir};
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{parse, parse_and_validate, Parse};
use property_bag::PropertyBag;
use state::Opacity;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
    ImageSurfaceDataExt,
    Pixel,
};

coord_units!(MaskUnits, CoordUnits::ObjectBoundingBox);
coord_units!(MaskContentUnits, CoordUnits::UserSpaceOnUse);

// remove this once cairo-rs has this mask_surface()
fn cairo_mask_surface(cr: &cairo::Context, surface: &cairo::Surface, x: f64, y: f64) {
    unsafe {
        let raw_cr = cr.to_glib_none().0;

        cairo_sys::cairo_mask_surface(raw_cr, surface.to_raw_none(), x, y);
    }
}

pub struct NodeMask {
    x: Cell<Length>,
    y: Cell<Length>,
    width: Cell<Length>,
    height: Cell<Length>,

    units: Cell<MaskUnits>,
    content_units: Cell<MaskContentUnits>,
}

impl NodeMask {
    pub fn new() -> NodeMask {
        NodeMask {
            x: Cell::new(NodeMask::get_default_pos(LengthDir::Horizontal)),
            y: Cell::new(NodeMask::get_default_pos(LengthDir::Vertical)),

            width: Cell::new(NodeMask::get_default_size(LengthDir::Horizontal)),
            height: Cell::new(NodeMask::get_default_size(LengthDir::Vertical)),

            units: Cell::new(MaskUnits::default()),
            content_units: Cell::new(MaskContentUnits::default()),
        }
    }

    fn get_default_pos(dir: LengthDir) -> Length {
        Length::parse_str("-10%", dir).unwrap()
    }

    fn get_default_size(dir: LengthDir) -> Length {
        Length::parse_str("120%", dir).unwrap()
    }

    pub fn generate_cairo_mask(
        &self,
        node: &RsvgNode,
        affine_before_mask: &cairo::Matrix,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<(), RenderingError> {
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        let width = draw_ctx.get_width() as i32;
        let height = draw_ctx.get_height() as i32;

        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;

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
                if let Some(ref rect) = draw_ctx.get_bbox().rect {
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

        let opacity = {
            let Opacity(o) = values.opacity;
            u8::from(o)
        };

        let mask_surface = compute_luminance_to_alpha(surface, opacity)?;

        let cr = draw_ctx.get_cairo_context();

        cr.identity_matrix();

        let (xofs, yofs) = draw_ctx.get_offset();
        cairo_mask_surface(&cr, &mask_surface, xofs, yofs);

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
    opacity: u8,
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

    let opacity = opacity as u32;

    let mut output = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;

    let output_stride = output.get_stride() as usize;

    {
        let mut output_data = output.get_data().unwrap();

        for (x, y, pixel) in Pixels::new(&surface, bounds) {
            //  Assuming, the pixel is linear RGB (not sRGB)
            //  y = luminance
            //  Y = 0.2126 R + 0.7152 G + 0.0722 B
            //  1.0 opacity = 255
            //
            //  When Y = 1.0, pixel for mask should be 0xFFFFFFFF
            //    (you get 1.0 luminance from 255 from R, G and B)
            //
            // r_mult = 0xFFFFFFFF / (255.0 * 255.0) * .2126 = 14042.45  ~= 14042
            // g_mult = 0xFFFFFFFF / (255.0 * 255.0) * .7152 = 47239.69  ~= 47240
            // b_mult = 0xFFFFFFFF / (255.0 * 255.0) * .0722 =  4768.88  ~= 4769
            //
            // This allows for the following expected behaviour:
            //    (we only care about the most sig byte)
            // if pixel = 0x00FFFFFF, pixel' = 0xFF......
            // if pixel = 0x00020202, pixel' = 0x02......
            // if pixel = 0x00000000, pixel' = 0x00......

            let r = pixel.r as u32;
            let g = pixel.g as u32;
            let b = pixel.b as u32;

            let output_pixel = Pixel {
                r: 0,
                g: 0,
                b: 0,
                a: (((r * 14042 + g * 47240 + b * 4769) * opacity) >> 24) as u8,
            };

            output_data.set_pixel(output_stride, output_pixel, x, y);
        }
    }

    Ok(output)
}

impl NodeTrait for NodeMask {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical)?),
                Attribute::Width => self.width.set(parse_and_validate(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    Length::check_nonnegative,
                )?),
                Attribute::Height => self.height.set(parse_and_validate(
                    "height",
                    value,
                    LengthDir::Vertical,
                    Length::check_nonnegative,
                )?),

                Attribute::MaskUnits => self.units.set(parse("maskUnits", value, ())?),

                Attribute::MaskContentUnits => {
                    self.content_units
                        .set(parse("maskContentUnits", value, ())?)
                }

                _ => (),
            }
        }

        Ok(())
    }
}
