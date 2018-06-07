use std::cell::{Cell, RefCell};
use std::ptr;

use cairo::{self, ImageSurface, MatrixTrait, Pattern};
use cairo_sys;
use glib;
use glib::translate::{from_glib_full, ToGlibPtr};
use glib_sys;
use libc;

use aspect_ratio::AspectRatio;
use attributes::Attribute;
use drawing_ctx;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgCNodeImpl, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::{Filter, FilterError, Primitive};

/// The `feImage` filter primitive.
pub struct Image {
    base: Primitive,
    aspect: Cell<AspectRatio>,
    href: RefCell<Option<String>>,

    // Storing this here seems hack-ish... It's required by rsvg_cairo_surface_new_from_href(). The
    // <image> element calls it in set_atts() but I don't think it belongs there.
    handle: Cell<*const RsvgHandle>,
}

impl Image {
    /// Constructs a new `Image` with empty properties.
    #[inline]
    pub fn new() -> Image {
        Image {
            base: Primitive::new::<Self>(),
            aspect: Cell::new(AspectRatio::default()),
            href: RefCell::new(None),

            handle: Cell::new(ptr::null()),
        }
    }

    /// Renders the filter if the source is an existing node.
    fn render_node(
        &self,
        ctx: &FilterContext,
        bounds: IRect,
        href: &str,
    ) -> Result<ImageSurface, FilterError> {
        // TODO: Port more of this to Rust.
        // Currently this is essentially a direct port of the C function.
        let drawable = drawing_ctx::get_acquired_node(ctx.drawing_context(), href)
            .ok_or(FilterError::InvalidInput)?;

        let surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().get_width(),
            ctx.source_graphic().get_height(),
        ).map_err(FilterError::OutputSurfaceCreation)?;

        drawing_ctx::get_cairo_context(ctx.drawing_context()).set_matrix(ctx.paffine());
        drawing_ctx::draw_node_on_surface(
            ctx.drawing_context(),
            &drawable.get(),
            &ctx.get_node_being_filtered(),
            &surface,
            f64::from(ctx.source_graphic().get_width()),
            f64::from(ctx.source_graphic().get_height()),
        );

        // Clip the output to bounds.
        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().get_width(),
            ctx.source_graphic().get_height(),
        ).map_err(FilterError::OutputSurfaceCreation)?;

        let cr = cairo::Context::new(&output_surface);
        cr.rectangle(
            f64::from(bounds.x0),
            f64::from(bounds.y0),
            f64::from(bounds.x1 - bounds.x0),
            f64::from(bounds.y1 - bounds.y0),
        );
        cr.clip();
        cr.set_source_surface(&surface, 0f64, 0f64);
        cr.paint();

        Ok(output_surface)
    }

    /// Renders the filter if the source is an external image.
    fn render_external_image(
        &self,
        ctx: &FilterContext,
        bounds: IRect,
        href: &str,
    ) -> Result<ImageSurface, FilterError> {
        let surface = {
            extern "C" {
                fn rsvg_cairo_surface_new_from_href(
                    handle: *const RsvgHandle,
                    href: *const libc::c_char,
                    error: *mut *mut glib_sys::GError,
                ) -> *mut cairo_sys::cairo_surface_t;
            }

            let mut error = ptr::null_mut();

            let raw_surface = unsafe {
                rsvg_cairo_surface_new_from_href(
                    self.handle.get(),
                    href.to_glib_none().0,
                    &mut error,
                )
            };
            if !raw_surface.is_null() {
                unsafe { cairo::ImageSurface::from_raw_full(raw_surface).unwrap() }
            } else {
                // TODO: pass the error through?
                let _: glib::Error = unsafe { from_glib_full(error) };
                return Err(FilterError::InvalidInput);
            }
        };

        let output_surface = ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().get_width(),
            ctx.source_graphic().get_height(),
        ).map_err(FilterError::OutputSurfaceCreation)?;

        let aspect = self.aspect.get();
        let (x, y, w, h) = aspect.compute(
            f64::from(surface.get_width()),
            f64::from(surface.get_height()),
            f64::from(bounds.x0),
            f64::from(bounds.y0),
            f64::from(bounds.x1 - bounds.x0),
            f64::from(bounds.y1 - bounds.y0),
        );

        if w != 0f64 && h != 0f64 {
            let ptn = cairo::SurfacePattern::create(&surface);
            let mut matrix = cairo::Matrix::new(
                w / f64::from(surface.get_width()),
                0f64,
                0f64,
                h / f64::from(surface.get_height()),
                x,
                y,
            );
            matrix.invert();
            ptn.set_matrix(matrix);

            let cr = cairo::Context::new(&output_surface);
            cr.rectangle(
                f64::from(bounds.x0),
                f64::from(bounds.y0),
                f64::from(bounds.x1 - bounds.x0),
                f64::from(bounds.y1 - bounds.y0),
            );
            cr.clip();
            cr.set_source(&ptn);
            cr.paint();
        }

        Ok(output_surface)
    }
}

impl NodeTrait for Image {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::PreserveAspectRatio => {
                    self.aspect
                        .set(parse("preserveAspectRatio", value, (), None)?)
                }

                // "path" is used by some older Adobe Illustrator versions
                Attribute::XlinkHref | Attribute::Path => {
                    drop(self.href.replace(Some(value.to_string())))
                }
                _ => (),
            }
        }

        self.handle.set(handle);

        Ok(())
    }

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
    }
}

impl Filter for Image {
    fn render(&self, _node: &RsvgNode, ctx: &FilterContext) -> Result<FilterResult, FilterError> {
        let href = self.href.borrow();
        let href = href.as_ref().ok_or(FilterError::InvalidInput)?;

        let bounds = self.base.get_bounds(ctx);

        let output_surface = match self.render_node(ctx, bounds, href) {
            Err(FilterError::InvalidInput) => self.render_external_image(ctx, bounds, href)?,
            Err(err) => return Err(err),
            Ok(surface) => surface,
        };

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
            output: FilterOutput {
                surface: output_surface,
                bounds,
            },
        })
    }
}
