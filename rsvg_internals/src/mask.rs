use cairo::{self, MatrixTrait};
use cairo_sys;
use glib::translate::*;
use std::cell::Cell;

use attributes::Attribute;
use coord_units::CoordUnits;
use draw;
use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;
use state::Opacity;

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
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    width: Cell<RsvgLength>,
    height: Cell<RsvgLength>,

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

    fn get_default_pos(dir: LengthDir) -> RsvgLength {
        RsvgLength::parse_str("-10%", dir).unwrap()
    }

    fn get_default_size(dir: LengthDir) -> RsvgLength {
        RsvgLength::parse_str("120%", dir).unwrap()
    }

    pub fn generate_cairo_mask(
        &self,
        node: &RsvgNode,
        affine_before_mask: &cairo::Matrix,
        draw_ctx: *mut RsvgDrawingCtx,
    ) {
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        let width = drawing_ctx::get_width(draw_ctx) as i32;
        let height = drawing_ctx::get_height(draw_ctx) as i32;

        let mut surface = match cairo::ImageSurface::create(cairo::Format::ARgb32, width, height) {
            Ok(surface) => surface,
            Err(_) => return,
        };

        let mask_units = CoordUnits::from(self.units.get());
        let content_units = CoordUnits::from(self.content_units.get());

        if mask_units == CoordUnits::ObjectBoundingBox {
            drawing_ctx::push_view_box(draw_ctx, 1.0, 1.0);
        }

        let x = self.x.get().normalize(&values, draw_ctx);
        let y = self.y.get().normalize(&values, draw_ctx);
        let w = self.width.get().normalize(&values, draw_ctx);
        let h = self.height.get().normalize(&values, draw_ctx);

        if mask_units == CoordUnits::ObjectBoundingBox {
            drawing_ctx::pop_view_box(draw_ctx);
        }

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let save_cr = drawing_ctx::get_cairo_context(draw_ctx);

            let mask_cr = cairo::Context::new(&surface);
            mask_cr.set_matrix(*affine_before_mask);
            mask_cr.transform(node.get_transform());

            drawing_ctx::set_cairo_context(draw_ctx, &mask_cr);

            if mask_units == CoordUnits::ObjectBoundingBox {
                let bbox = drawing_ctx::get_bbox(draw_ctx);
                let rect = bbox.rect.unwrap();

                draw::add_clipping_rect(
                    draw_ctx,
                    x * rect.width + rect.x,
                    y * rect.height + rect.y,
                    w * rect.width,
                    h * rect.height,
                );
            } else {
                draw::add_clipping_rect(draw_ctx, x, y, w, h);
            }

            if content_units == CoordUnits::ObjectBoundingBox {
                let bbox = drawing_ctx::get_bbox(draw_ctx);
                let rect = bbox.rect.unwrap();
                let bbtransform =
                    cairo::Matrix::new(rect.width, 0.0, 0.0, rect.height, rect.x, rect.y);

                mask_cr.transform(bbtransform);

                drawing_ctx::push_view_box(draw_ctx, 1.0, 1.0);
            }

            node.draw_children(node, &cascaded, draw_ctx, true, false);

            if content_units == CoordUnits::ObjectBoundingBox {
                drawing_ctx::pop_view_box(draw_ctx);
            }

            drawing_ctx::set_cairo_context(draw_ctx, &save_cr);
        }

        {
            let rowstride = surface.get_stride() as usize;
            let mut pixels = surface.get_data().unwrap();
            let opacity = {
                let Opacity(o) = values.opacity;
                u8::from(o)
            };

            for row in pixels.chunks_mut(rowstride) {
                for p in row[..4 * width as usize].chunks_mut(4) {
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
                    //
                    // NOTE: the following assumes little-endian
                    let (r, g, b, o) = (p[2] as u32, p[1] as u32, p[0] as u32, opacity as u32);
                    p[3] = (((r * 14042 + g * 47240 + b * 4769) * o) >> 24) as u8;
                }
            }
        }

        let cr = drawing_ctx::get_cairo_context(draw_ctx);

        cr.identity_matrix();

        let (xofs, yofs) = drawing_ctx::get_offset(draw_ctx);
        cairo_mask_surface(&cr, &surface, xofs, yofs);
    }
}

impl NodeTrait for NodeMask {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal, None)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical, None)?),
                Attribute::Width => self.width.set(parse(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    Some(RsvgLength::check_nonnegative),
                )?),
                Attribute::Height => self.height.set(parse(
                    "height",
                    value,
                    LengthDir::Vertical,
                    Some(RsvgLength::check_nonnegative),
                )?),

                Attribute::MaskUnits => self.units.set(parse("maskUnits", value, (), None)?),

                Attribute::MaskContentUnits => {
                    self.content_units
                        .set(parse("maskContentUnits", value, (), None)?)
                }

                _ => (),
            }
        }

        Ok(())
    }
}
