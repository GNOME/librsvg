use cairo;
use cairo::{MatrixTrait, Pattern};
use cairo_sys;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use std::cell::{Cell, RefCell};
use std::ptr;

use aspect_ratio::AspectRatio;
use attributes::Attribute;
use bbox::BoundingBox;
use drawing_ctx::DrawingCtx;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::{parse, parse_and_validate};
use property_bag::PropertyBag;

pub struct NodeImage {
    aspect: Cell<AspectRatio>,
    x: Cell<Length>,
    y: Cell<Length>,
    w: Cell<Length>,
    h: Cell<Length>,
    surface: RefCell<Option<cairo::ImageSurface>>,
}

impl NodeImage {
    pub fn new() -> NodeImage {
        NodeImage {
            aspect: Cell::new(AspectRatio::default()),
            x: Cell::new(Length::default()),
            y: Cell::new(Length::default()),
            w: Cell::new(Length::default()),
            h: Cell::new(Length::default()),
            surface: RefCell::new(None),
        }
    }
}

impl NodeTrait for NodeImage {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        // SVG element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical)?),
                Attribute::Width => self.w.set(parse_and_validate(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    Length::check_nonnegative,
                )?),
                Attribute::Height => self.h.set(parse_and_validate(
                    "height",
                    value,
                    LengthDir::Vertical,
                    Length::check_nonnegative,
                )?),

                Attribute::PreserveAspectRatio => {
                    self.aspect.set(parse("preserveAspectRatio", value, ())?)
                }

                Attribute::XlinkHref | Attribute::Path => {
                    // "path" is used by some older Adobe Illustrator versions

                    extern "C" {
                        fn rsvg_cairo_surface_new_from_href(
                            handle: *const RsvgHandle,
                            href: *const libc::c_char,
                            error: *mut *mut glib_sys::GError,
                        ) -> *mut cairo_sys::cairo_surface_t;
                    }

                    let mut error = ptr::null_mut();

                    let raw_surface = unsafe {
                        rsvg_cairo_surface_new_from_href(handle, value.to_glib_none().0, &mut error)
                    };
                    if !raw_surface.is_null() {
                        *self.surface.borrow_mut() = Some(unsafe {
                            cairo::ImageSurface::from_raw_full(raw_surface).unwrap()
                        });
                    } else {
                        let _: glib::Error = unsafe { from_glib_full(error) }; // FIXME: we should note that the image couldn't be loaded
                    }
                }

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
        let values = cascaded.get();

        if let Some(ref surface) = *self.surface.borrow() {
            let x = self.x.get().normalize(values, draw_ctx);
            let y = self.y.get().normalize(values, draw_ctx);
            let w = self.w.get().normalize(values, draw_ctx);
            let h = self.h.get().normalize(values, draw_ctx);

            draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
                let aspect = self.aspect.get();

                if !values.is_overflow() && aspect.is_slice() {
                    dc.clip(x, y, w, h);
                }

                // The bounding box for <image> is decided by the values of x, y, w, h and not by
                // the final computed image bounds.
                let bbox = BoundingBox::new(&dc.get_cairo_context().get_matrix()).with_rect(Some(
                    cairo::Rectangle {
                        x,
                        y,
                        width: w,
                        height: h,
                    },
                ));

                let width = surface.get_width();
                let height = surface.get_height();
                if clipping || width == 0 || height == 0 {
                    return;
                }

                let width = f64::from(width);
                let height = f64::from(height);

                let (x, y, w, h) = aspect.compute(width, height, x, y, w, h);

                let cr = dc.get_cairo_context();

                cr.save();

                dc.set_affine_on_cr(&cr);
                cr.scale(w / width, h / height);
                let x = x * width / w;
                let y = y * height / h;

                cr.set_operator(cairo::Operator::from(values.comp_op));

                // We need to set extend appropriately, so can't use cr.set_source_surface().
                //
                // If extend is left at its default value (None), then bilinear scaling uses
                // transparency outside of the image producing incorrect results.
                // For example, in svg1.1/filters-blend-01-b.svgthere's a completely
                // opaque 100×1 image of a gradient scaled to 100×98 which ends up
                // transparent almost everywhere without this fix (which it shouldn't).
                let ptn = cairo::SurfacePattern::create(&surface);
                let mut matrix = cairo::Matrix::identity();
                matrix.translate(-x, -y);
                ptn.set_matrix(matrix);
                ptn.set_extend(cairo::Extend::Pad);
                cr.set_source(&ptn);

                // Clip is needed due to extend being set to pad.
                cr.rectangle(x, y, width, height);
                cr.clip();

                cr.paint();

                cr.restore();

                dc.insert_bbox(&bbox);
            });
        }
    }
}
