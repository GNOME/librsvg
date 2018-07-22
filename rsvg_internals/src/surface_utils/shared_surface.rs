//! Shared access to Cairo image surfaces.
use std::ptr::NonNull;

use cairo::prelude::SurfaceExt;
use cairo::{self, ImageSurface};
use cairo_sys;
use glib::translate::{Stash, ToGlibPtr};
use rulinalg::matrix::{BaseMatrix, Matrix};

use filters::context::IRect;
use srgb;
use util::clamp;

use super::{
    iterators::{PixelRectangle, Pixels},
    EdgeMode,
    ImageSurfaceDataExt,
    Pixel,
};

/// Types of pixel data in a `SharedImageSurface`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum SurfaceType {
    /// The pixel data is in the sRGB color space.
    SRgb,
    /// The pixel data is in the linear sRGB color space.
    LinearRgb,
    /// The pixel data is alpha-only (contains meaningful data only in the alpha channel).
    ///
    /// A number of methods are optimized for alpha-only surfaces. For example, linearization and
    /// unlinearization have no effect for alpha-only surfaces.
    AlphaOnly,
}

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

    surface_type: SurfaceType,
}

impl SharedImageSurface {
    /// Creates a `SharedImageSurface` from a unique `ImageSurface`.
    ///
    /// # Panics
    /// Panics if the surface format isn't `ARgb32` and if the surface is not unique, that is, its
    /// reference count isn't 1.
    #[inline]
    pub fn new(surface: ImageSurface, surface_type: SurfaceType) -> Result<Self, cairo::Status> {
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
            surface_type,
        })
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
        self.surface_type == SurfaceType::AlphaOnly
    }

    /// Returns the type of this surface.
    #[inline]
    pub fn surface_type(&self) -> SurfaceType {
        self.surface_type
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

    /// Retrieves the pixel value if it is within `bounds`, otherwise returns a transparent black
    /// pixel.
    #[inline]
    pub fn get_pixel_or_transparent(&self, bounds: IRect, x: i32, y: i32) -> Pixel {
        if bounds.contains(x, y) {
            self.get_pixel(x as u32, y as u32)
        } else {
            Pixel {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            }
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

        SharedImageSurface::new(output_surface, self.surface_type)
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

        SharedImageSurface::new(output_surface, SurfaceType::AlphaOnly)
    }

    /// Returns a surface with pre-multiplication of color values undone.
    ///
    /// HACK: this is storing unpremultiplied pixels in an ARGB32 image surface (which is supposed
    /// to be premultiplied pixels).
    pub fn unpremultiply(&self, bounds: IRect) -> Result<SharedImageSurface, cairo::Status> {
        // Unpremultiplication doesn't affect the alpha channel.
        if self.is_alpha_only() {
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

        SharedImageSurface::new(output_surface, self.surface_type)
    }

    /// Converts the surface to the linear sRGB color space.
    #[inline]
    pub fn to_linear_rgb(&self, bounds: IRect) -> Result<SharedImageSurface, cairo::Status> {
        if self.surface_type == SurfaceType::LinearRgb {
            Ok(self.clone())
        } else {
            srgb::linearize_surface(self, bounds)
        }
    }

    /// Converts the surface to the sRGB color space.
    #[inline]
    pub fn to_srgb(&self, bounds: IRect) -> Result<SharedImageSurface, cairo::Status> {
        if self.surface_type == SurfaceType::SRgb {
            Ok(self.clone())
        } else {
            srgb::unlinearize_surface(self, bounds)
        }
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

            if self.is_alpha_only() {
                for (x, y, _pixel) in Pixels::new(self, bounds) {
                    let kernel_bounds = IRect {
                        x0: x as i32 - target.0,
                        y0: y as i32 - target.1,
                        x1: x as i32 - target.0 + kernel.cols() as i32,
                        y1: y as i32 - target.1 + kernel.rows() as i32,
                    };

                    let mut a = 0.0;

                    for (x, y, pixel) in PixelRectangle::new(self, bounds, kernel_bounds, edge_mode)
                    {
                        let kernel_x = (kernel_bounds.x1 - x - 1) as usize;
                        let kernel_y = (kernel_bounds.y1 - y - 1) as usize;
                        let factor = kernel[[kernel_y, kernel_x]];

                        a += f64::from(pixel.a) * factor;
                    }

                    let convert = |x: f64| clamp(x, 0.0, 255.0).round() as u8;

                    let output_pixel = Pixel {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: convert(a),
                    };

                    output_data.set_pixel(output_stride, output_pixel, x, y);
                }
            } else {
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

                    for (x, y, pixel) in PixelRectangle::new(self, bounds, kernel_bounds, edge_mode)
                    {
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
        }

        SharedImageSurface::new(output_surface, self.surface_type)
    }

    /// Performs a horizontal or vertical box blur.
    ///
    /// The `target` parameter determines the position of the kernel relative to each pixel of the
    /// image. The value of `0` indicates that the first pixel of the kernel corresponds to the
    /// current pixel, and the rest of the kernel is to the right or bottom of the pixel. The value
    /// of `kernel_size / 2` centers a kernel with an odd size.
    ///
    /// # Panics
    /// Panics if `kernel_size` is `0` or if `target >= kernel_size`.
    // This is public (and not inlined into box_blur()) for the purpose of accessing it from the
    // benchmarks.
    pub fn box_blur_loop(
        &self,
        output_surface: &mut cairo::ImageSurface,
        bounds: IRect,
        kernel_size: usize,
        target: usize,
        vertical: bool,
    ) {
        assert_ne!(kernel_size, 0);
        assert!(target < kernel_size);

        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            // Shift is target into the opposite direction.
            let shift = (kernel_size - target) as i32;
            let target = target as i32;

            // Convert to f64 once since we divide by it.
            let kernel_size_f64 = kernel_size as f64;
            let compute = |x: u32| (f64::from(x) / kernel_size_f64 + 0.5) as u8;

            // Depending on `vertical`, we're blurring either horizontally line-by-line, or
            // vertically column-by-column. In the code below, the main axis is the
            // axis along which the blurring happens (so if `vertical` is false, the
            // main axis is the horizontal axis). The other axis is the outer loop
            // axis. The code uses `i` and `j` for the other axis and main axis
            // coordinates, respectively.
            let (main_axis_min, main_axis_max, other_axis_min, other_axis_max) = if vertical {
                (bounds.y0, bounds.y1, bounds.x0, bounds.x1)
            } else {
                (bounds.x0, bounds.x1, bounds.y0, bounds.y1)
            };

            // Helper functions for getting and setting the pixels.
            let pixel = |i, j| {
                let (x, y) = if vertical { (i, j) } else { (j, i) };

                self.get_pixel_or_transparent(bounds, x, y)
            };

            let mut set_pixel = |i, j, pixel| {
                let (x, y) = if vertical { (i, j) } else { (j, i) };

                output_data.set_pixel(output_stride, pixel, x, y);
            };

            for i in other_axis_min..other_axis_max {
                // The idea is that since all weights of the box blur kernel are equal, for each
                // step along the main axis, instead of recomputing the full sum,
                // we can take the previous sum, subtract the "oldest" pixel value
                // and add the "newest" pixel value.
                //
                // The sum is u32 so that it can fit MAXIMUM_KERNEL_SIZE * 255.
                let mut sum_r = 0;
                let mut sum_g = 0;
                let mut sum_b = 0;
                let mut sum_a = 0;

                // The whole sum needs to be computed for the first pixel. However, we know that
                // values outside of bounds are transparent, so the loop starts on
                // the first pixel in bounds.
                for j in main_axis_min..main_axis_min + shift {
                    let Pixel { r, g, b, a } = pixel(i, j);

                    if !self.is_alpha_only() {
                        sum_r += u32::from(r);
                        sum_g += u32::from(g);
                        sum_b += u32::from(b);
                    }

                    sum_a += u32::from(a);
                }

                set_pixel(
                    i as u32,
                    main_axis_min as u32,
                    Pixel {
                        r: compute(sum_r),
                        g: compute(sum_g),
                        b: compute(sum_b),
                        a: compute(sum_a),
                    },
                );

                // Now, go through all the other pixels.
                for j in main_axis_min + 1..main_axis_max {
                    let old_pixel = pixel(i, j - target - 1);

                    if !self.is_alpha_only() {
                        sum_r -= u32::from(old_pixel.r);
                        sum_g -= u32::from(old_pixel.g);
                        sum_b -= u32::from(old_pixel.b);
                    }

                    sum_a -= u32::from(old_pixel.a);

                    let new_pixel = pixel(i, j + shift - 1);

                    if !self.is_alpha_only() {
                        sum_r += u32::from(new_pixel.r);
                        sum_g += u32::from(new_pixel.g);
                        sum_b += u32::from(new_pixel.b);
                    }

                    sum_a += u32::from(new_pixel.a);

                    set_pixel(
                        i as u32,
                        j as u32,
                        Pixel {
                            r: compute(sum_r),
                            g: compute(sum_g),
                            b: compute(sum_b),
                            a: compute(sum_a),
                        },
                    );
                }
            }
        }
    }

    /// Performs a horizontal or vertical box blur.
    ///
    /// The `target` parameter determines the position of the kernel relative to each pixel of the
    /// image. The value of `0` indicates that the first pixel of the kernel corresponds to the
    /// current pixel, and the rest of the kernel is to the right or bottom of the pixel. The value
    /// of `kernel_size / 2` centers a kernel with an odd size.
    ///
    /// # Panics
    /// Panics if `kernel_size` is `0` or if `target >= kernel_size`.
    #[inline]
    pub fn box_blur(
        &self,
        bounds: IRect,
        kernel_size: usize,
        target: usize,
        vertical: bool,
    ) -> Result<SharedImageSurface, cairo::Status> {
        let mut output_surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)?;

        self.box_blur_loop(&mut output_surface, bounds, kernel_size, target, vertical);

        SharedImageSurface::new(output_surface, self.surface_type)
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
