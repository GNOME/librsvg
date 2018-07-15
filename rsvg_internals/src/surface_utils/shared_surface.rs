//! Shared access to Cairo image surfaces.
use std::ptr::NonNull;

use cairo::prelude::SurfaceExt;
use cairo::{self, ImageSurface};
use cairo_sys;
use glib::translate::{Stash, ToGlibPtr};
use rulinalg::matrix::{BaseMatrix, Matrix};

use filters::context::IRect;
use util::clamp;

use super::{
    iterators::{PixelRectangle, Pixels},
    EdgeMode,
    ImageSurfaceDataExt,
    Pixel,
};

/// Wrapper for a Cairo image surface that allows shared access.
///
/// There doesn't seem to be any good way of making safe shared access to `ImageSurface` pixel
/// data, since a read-only borrowed reference can still be cloned and then modified (for example,
/// via a `Context`). We can't simply use `ImageSurface::get_data()` because in the filter code we
/// have surfaces referenced from multiple places and it would probably add more complexity to
/// remove that and start passing around references.
///
/// This wrapper asserts the uniqueness of its image surface and doesn't permit modifying it.
///
/// Note: originally I had an idea of using `Rc<RefCell<ImageSurface>>` here which allows to create
/// both read-only and unique read-write accessors safely, however then I realized a read-write
/// accessor isn't of much use if it can't expose a Cairo context interface. Cairo contexts have
/// the very same issue that they can be cloned from a read-only reference and break all safety
/// constraints in this way. Thus the only safe way of exposing a Cairo context seemed to be to
/// manually add all Cairo context methods on the accessor forwarding to the underlying Cairo
/// context (without exposing the context itself to prevent cloning), which would result in too
/// much code. Unless it's absolutely required, I'd like to avoid that.
///
/// Having just read-only access simplifies things further dropping the need for `Rc<RefCell<>>`
/// altogether.
#[derive(Debug, Clone)]
pub struct SharedImageSurface {
    surface: ImageSurface,

    data_ptr: NonNull<u8>, // *const.
    width: i32,
    height: i32,
    stride: isize,

    /// Whether this surface contains meaningful data only in the alpha channel.
    ///
    /// This is used for optimizations, particularly in `convolve()` to skip processing other
    /// channels.
    alpha_only: bool,
}

impl SharedImageSurface {
    /// Creates a `SharedImageSurface` from a unique `ImageSurface`.
    ///
    /// # Panics
    /// Panics if the surface format isn't `ARgb32` and if the surface is not unique, that is, its
    /// reference count isn't 1.
    #[inline]
    pub fn new(surface: ImageSurface) -> Result<Self, cairo::Status> {
        // get_pixel() assumes ARgb32.
        assert_eq!(surface.get_format(), cairo::Format::ARgb32);

        let reference_count =
            unsafe { cairo_sys::cairo_surface_get_reference_count(surface.to_raw_none()) };
        assert_eq!(reference_count, 1);

        surface.flush();
        if surface.status() != cairo::Status::Success {
            return Err(surface.status());
        }

        let data_ptr = NonNull::new(unsafe {
            cairo_sys::cairo_image_surface_get_data(surface.to_raw_none())
        }).unwrap();

        let width = surface.get_width();
        let height = surface.get_height();
        let stride = surface.get_stride() as isize;

        Ok(Self {
            surface,
            data_ptr,
            width,
            height,
            stride,
            alpha_only: false,
        })
    }

    /// Creates a `SharedImageSurface` from a unique `ImageSurface` with meaningful data only in
    /// the alpha channel.
    ///
    /// # Panics
    /// Panics if the surface format isn't `ARgb32` and if the surface is not unique, that is, its
    /// reference count isn't 1.
    #[inline]
    pub fn new_alpha_only(surface: ImageSurface) -> Result<Self, cairo::Status> {
        let mut rv = Self::new(surface)?;
        rv.alpha_only = true;
        Ok(rv)
    }

    /// Converts this `SharedImageSurface` back into a Cairo image surface.
    #[inline]
    pub fn into_image_surface(self) -> Result<ImageSurface, cairo::Status> {
        let reference_count =
            unsafe { cairo_sys::cairo_surface_get_reference_count(self.surface.to_raw_none()) };

        if reference_count == 1 {
            Ok(self.surface)
        } else {
            // If there are any other references, copy the underlying surface.
            let bounds = IRect {
                x0: 0,
                y0: 0,
                x1: self.width,
                y1: self.height,
            };

            self.copy_surface(bounds)
        }
    }

    /// Returns the surface width.
    #[inline]
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Returns the surface height.
    #[inline]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Returns `true` if the surface contains meaningful data only in the alpha channel.
    #[inline]
    pub fn is_alpha_only(&self) -> bool {
        self.alpha_only
    }

    /// Retrieves the pixel value at the given coordinates.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Pixel {
        assert!(x < self.width as u32);
        assert!(y < self.height as u32);

        let value = unsafe {
            *(self
                .data_ptr
                .as_ptr()
                .offset(y as isize * self.stride + x as isize * 4) as *const u32)
        };

        Pixel {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
            a: ((value >> 24) & 0xFF) as u8,
        }
    }

    /// Calls `set_source_surface()` on the given Cairo context.
    #[inline]
    pub fn set_as_source_surface(&self, cr: &cairo::Context, x: f64, y: f64) {
        cr.set_source_surface(&self.surface, x, y);
    }

    /// Returns a new `ImageSurface` with the same contents as the one stored in this
    /// `SharedImageSurface` within the given bounds.
    pub fn copy_surface(&self, bounds: IRect) -> Result<ImageSurface, cairo::Status> {
        let output_surface = ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)?;

        let cr = cairo::Context::new(&output_surface);
        cr.rectangle(
            bounds.x0 as f64,
            bounds.y0 as f64,
            (bounds.x1 - bounds.x0) as f64,
            (bounds.y1 - bounds.y0) as f64,
        );
        cr.clip();

        cr.set_source_surface(&self.surface, 0f64, 0f64);
        cr.paint();

        Ok(output_surface)
    }

    /// Scales the given surface by `x` and `y` into a surface `width`×`height` in size, clipped by
    /// `bounds`.
    pub fn scale_to(
        &self,
        width: i32,
        height: i32,
        bounds: IRect,
        x: f64,
        y: f64,
    ) -> Result<SharedImageSurface, cairo::Status> {
        let output_surface = ImageSurface::create(cairo::Format::ARgb32, width, height)?;

        {
            let cr = cairo::Context::new(&output_surface);
            cr.rectangle(
                bounds.x0 as f64,
                bounds.y0 as f64,
                (bounds.x1 - bounds.x0) as f64,
                (bounds.y1 - bounds.y0) as f64,
            );
            cr.clip();

            cr.scale(x, y);
            self.set_as_source_surface(&cr, 0.0, 0.0);
            cr.paint();
        }

        if self.alpha_only {
            SharedImageSurface::new_alpha_only(output_surface)
        } else {
            SharedImageSurface::new(output_surface)
        }
    }

    /// Returns a scaled version of a surface and bounds.
    #[inline]
    pub fn scale(
        &self,
        bounds: IRect,
        x: f64,
        y: f64,
    ) -> Result<(SharedImageSurface, IRect), cairo::Status> {
        let new_width = (f64::from(self.width) * x).ceil() as i32;
        let new_height = (f64::from(self.height) * y).ceil() as i32;
        let new_bounds = bounds.scale(x, y);

        Ok((
            self.scale_to(new_width, new_height, new_bounds, x, y)?,
            new_bounds,
        ))
    }

    /// Returns a surface with black background and alpha channel matching this surface.
    pub fn extract_alpha(&self, bounds: IRect) -> Result<SharedImageSurface, cairo::Status> {
        let mut output_surface =
            ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, Pixel { a, .. }) in Pixels::new(self, bounds) {
                let output_pixel = Pixel {
                    r: 0,
                    g: 0,
                    b: 0,
                    a,
                };
                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        SharedImageSurface::new_alpha_only(output_surface)
    }

    /// Returns a surface with pre-multiplication of color values undone.
    ///
    /// HACK: this is storing unpremultiplied pixels in an ARGB32 image surface (which is supposed
    /// to be premultiplied pixels).
    pub fn unpremultiply(&self, bounds: IRect) -> Result<SharedImageSurface, cairo::Status> {
        // Unpremultiplication doesn't affect the alpha channel.
        if self.alpha_only {
            return Ok(self.clone());
        }

        let mut output_surface =
            ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)?;

        let stride = output_surface.get_stride() as usize;
        {
            let mut data = output_surface.get_data().unwrap();

            for (x, y, pixel) in Pixels::new(self, bounds) {
                data.set_pixel(stride, pixel.unpremultiply(), x, y);
            }
        }

        SharedImageSurface::new(output_surface)
    }

    /// Performs a convolution.
    ///
    /// Note that `kernel` is rotated 180 degrees.
    ///
    /// The `target` parameter determines the position of the kernel relative to each pixel of the
    /// image. The value of `(0, 0)` indicates that the top left pixel of the (180-degrees-rotated)
    /// kernel corresponds to the current pixel, and the rest of the kernel is to the right and
    /// bottom of the pixel. The value of `(cols / 2, rows / 2)` centers a kernel with an odd
    /// number of rows and columns.
    ///
    /// # Panics
    /// Panics if `kernel` has zero rows or columns.
    pub fn convolve(
        &self,
        bounds: IRect,
        target: (i32, i32),
        kernel: &Matrix<f64>,
        edge_mode: EdgeMode,
    ) -> Result<SharedImageSurface, cairo::Status> {
        assert!(kernel.rows() >= 1);
        assert!(kernel.cols() >= 1);

        let mut output_surface =
            ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)?;

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, _pixel) in Pixels::new(self, bounds) {
                let kernel_bounds = IRect {
                    x0: x as i32 - target.0,
                    y0: y as i32 - target.1,
                    x1: x as i32 - target.0 + kernel.cols() as i32,
                    y1: y as i32 - target.1 + kernel.rows() as i32,
                };

                let mut r = 0.0;
                let mut g = 0.0;
                let mut b = 0.0;
                let mut a = 0.0;

                for (x, y, pixel) in PixelRectangle::new(self, bounds, kernel_bounds, edge_mode) {
                    let kernel_x = (kernel_bounds.x1 - x - 1) as usize;
                    let kernel_y = (kernel_bounds.y1 - y - 1) as usize;
                    let factor = kernel[[kernel_y, kernel_x]];

                    r += f64::from(pixel.r) * factor;
                    g += f64::from(pixel.g) * factor;
                    b += f64::from(pixel.b) * factor;
                    a += f64::from(pixel.a) * factor;
                }

                let convert = |x: f64| clamp(x, 0.0, 255.0).round() as u8;

                let output_pixel = Pixel {
                    r: convert(r),
                    g: convert(g),
                    b: convert(b),
                    a: convert(a),
                };

                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        SharedImageSurface::new(output_surface)
    }

    /// Returns a raw pointer to the underlying surface.
    ///
    /// # Safety
    /// The returned pointer must not be used to modify the surface.
    #[inline]
    pub unsafe fn to_glib_none(&self) -> Stash<*mut cairo_sys::cairo_surface_t, ImageSurface> {
        self.surface.to_glib_none()
    }
}
