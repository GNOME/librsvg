use libc;
use cairo;
use cairo_sys;
use glib;
use glib::translate::*;
use glib_sys;
use std::cell::{Cell, RefCell};
use std::ptr;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;

use aspect_ratio::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use property_bag;
use property_bag::RsvgPropertyBag;

struct NodeImage {
    aspect:  Cell<AspectRatio>,
    x:       Cell<RsvgLength>,
    y:       Cell<RsvgLength>,
    w:       Cell<RsvgLength>,
    h:       Cell<RsvgLength>,
    surface: RefCell<Option<cairo::ImageSurface>>
}

impl NodeImage {
    fn new () -> NodeImage {
        NodeImage {
            aspect:  Cell::new (AspectRatio::default ()),
            x:       Cell::new (RsvgLength::default ()),
            y:       Cell::new (RsvgLength::default ()),
            w:       Cell::new (RsvgLength::default ()),
            h:       Cell::new (RsvgLength::default ()),
            surface: RefCell::new (None)
        }
    }
}

impl NodeTrait for NodeImage {
    fn set_atts (&self, _: &RsvgNode, handle: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        self.x.set (property_bag::parse_or_default (pbag, "x", LengthDir::Horizontal, None)?);
        self.y.set (property_bag::parse_or_default (pbag, "y", LengthDir::Vertical, None)?);
        self.w.set (property_bag::parse_or_default (pbag, "width", LengthDir::Horizontal,
                                                    Some(RsvgLength::check_nonnegative))?);
        self.h.set (property_bag::parse_or_default (pbag, "height", LengthDir::Vertical,
                                                    Some(RsvgLength::check_nonnegative))?);
        self.aspect.set (property_bag::parse_or_default (pbag, "preserveAspectRatio", (), None)?);

        let mut href = property_bag::lookup (pbag, "xlink:href");

        if href.is_none() {
            // "path" is used by some older adobe illustrator versions
            href = property_bag::lookup (pbag, "path");
        }

        if let Some(href) = href {
            extern "C" { fn rsvg_cairo_surface_new_from_href
                         (handle: *const RsvgHandle,
                          href:   *const libc::c_char,
                          error:  *mut *mut glib_sys::GError) -> *mut cairo_sys::cairo_surface_t;
            }

            let mut error = ptr::null_mut();

            let raw_surface = unsafe { rsvg_cairo_surface_new_from_href (handle,
                                                                         href.to_glib_none().0,
                                                                         &mut error) };
            if !raw_surface.is_null() {
                *self.surface.borrow_mut() = Some(unsafe { cairo::ImageSurface::from_raw_full (raw_surface).unwrap() });
            } else {
                let _: glib::Error = unsafe { from_glib_full(error) }; // FIXME: we should note that the image couldn't be loaded
            }
        }

        Ok (())
    }

    fn draw (&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        if let Some(ref surface) = *self.surface.borrow() {
            let x = self.x.get().normalize(draw_ctx);
            let y = self.y.get().normalize(draw_ctx);
            let w = self.w.get().normalize(draw_ctx);
            let h = self.h.get().normalize(draw_ctx);

            let state = node.get_state();

            drawing_ctx::state_reinherit_top(draw_ctx, state, dominate);
            drawing_ctx::push_discrete_layer(draw_ctx);

            let aspect = self.aspect.get();

            if !drawing_ctx::state_is_overflow(state) {
                match aspect.align {
                    Align::Aligned { align: _,
                                     fit: FitMode::Slice } => {
                        drawing_ctx::add_clipping_rect(draw_ctx, x, y, w, h);
                    },

                    _ => ()
                }
            }

            let (x, y, w, h) = aspect.compute (f64::from(surface.get_width()),
                                               f64::from(surface.get_height()),
                                               x, y, w, h);

            drawing_ctx::render_surface(draw_ctx, &surface, x, y, w, h);

            drawing_ctx::pop_discrete_layer(draw_ctx);
        }
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

#[no_mangle]
pub extern fn rsvg_node_image_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Image,
                    raw_parent,
                    Box::new (NodeImage::new ()))
}
