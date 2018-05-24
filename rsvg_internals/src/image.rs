use cairo;
use cairo_sys;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use std::cell::{Cell, RefCell};
use std::ptr;

use aspect_ratio::AspectRatio;
use attributes::Attribute;
use draw::{add_clipping_rect, draw_surface};
use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::parse;
use property_bag::PropertyBag;

struct NodeImage {
    aspect: Cell<AspectRatio>,
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    w: Cell<RsvgLength>,
    h: Cell<RsvgLength>,
    surface: RefCell<Option<cairo::ImageSurface>>,
}

impl NodeImage {
    fn new() -> NodeImage {
        NodeImage {
            aspect: Cell::new(AspectRatio::default()),
            x: Cell::new(RsvgLength::default()),
            y: Cell::new(RsvgLength::default()),
            w: Cell::new(RsvgLength::default()),
            h: Cell::new(RsvgLength::default()),
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
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal, None)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical, None)?),
                Attribute::Width => self.w.set(parse(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    Some(RsvgLength::check_nonnegative),
                )?),
                Attribute::Height => self.h.set(parse(
                    "height",
                    value,
                    LengthDir::Vertical,
                    Some(RsvgLength::check_nonnegative),
                )?),

                Attribute::PreserveAspectRatio => {
                    self.aspect
                        .set(parse("preserveAspectRatio", value, (), None)?)
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

    fn draw(&self, node: &RsvgNode, draw_ctx: *mut RsvgDrawingCtx, _dominate: i32, clipping: bool) {
        let values = &node.get_computed_values();

        if let Some(ref surface) = *self.surface.borrow() {
            let x = self.x.get().normalize(draw_ctx);
            let y = self.y.get().normalize(draw_ctx);
            let w = self.w.get().normalize(draw_ctx);
            let h = self.h.get().normalize(draw_ctx);

            drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);

            let aspect = self.aspect.get();

            if !values.is_overflow() && aspect.is_slice() {
                add_clipping_rect(draw_ctx, &values.affine, x, y, w, h);
            }

            let (x, y, w, h) = aspect.compute(
                f64::from(surface.get_width()),
                f64::from(surface.get_height()),
                x,
                y,
                w,
                h,
            );

            draw_surface(draw_ctx, values, surface, x, y, w, h, clipping);

            drawing_ctx::pop_discrete_layer(draw_ctx, values, clipping);
        }
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_image_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Image, raw_parent, Box::new(NodeImage::new()))
}
