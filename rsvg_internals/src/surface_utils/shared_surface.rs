//! Shared access to Cairo image surfaces.
use std::cmp::min;
use std::marker::PhantomData;
use std::ptr::NonNull;

use cairo::prelude::SurfaceExt;
use cairo::{self, ImageSurface};
use cairo_sys;
use gdk_pixbuf::{Colorspace, Pixbuf, PixbufExt};
use glib::translate::{Stash, ToGlibPtr};
use nalgebra::{storage::Storage, Dim, Matrix};
use rayon;

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

// The access is read-only, the ref-counting on an `ImageSurface` is atomic.
unsafe impl Sync for SharedImageSurface {}

/// A compile-time blur direction variable.
pub trait BlurDirection {
    const IS_VERTICAL: bool;
}

/// Vertical blur direction.
pub enum Vertical {}
/// Horizontal blur direction.
pub enum Horizontal {}

impl BlurDirection for Vertical {
    const IS_VERTICAL: bool = true;
}

impl BlurDirection for Horizontal {
    const IS_VERTICAL: bool = false;
}

/// A compile-time alpha-only marker variable.
pub trait IsAlphaOnly {
    const IS_ALPHA_ONLY: bool;
}

/// Alpha-only.
pub enum AlphaOnly {}
/// Not alpha-only.
pub enum NotAlphaOnly {}

impl IsAlphaOnly for AlphaOnly {
    const IS_ALPHA_ONLY: bool = true;
}

impl IsAlphaOnly for NotAlphaOnly {
    const IS_ALPHA_ONLY: bool = false;
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

        let data_ptr =
            NonNull::new(unsafe { cairo_sys::cairo_image_surface_get_data(surface.to_raw_none()) })
                .unwrap();

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

    pub fn from_pixbuf(pixbuf: &Pixbuf) -> Result<SharedImageSurface, cairo::Status> {
        assert!(pixbuf.get_colorspace() == Colorspace::Rgb);

        let n_channels = pixbuf.get_n_channels();
        assert!(n_channels == 3 || n_channels == 4);
        let has_alpha = n_channels == 4;

        let width = pixbuf.get_width();
        assert!(width > 0);

        let height = pixbuf.get_height();
        assert!(height > 0);

        let pixbuf_stride = pixbuf.get_rowstride();
        assert!(pixbuf_stride > 0);
        let pixbuf_stride = pixbuf_stride as usize;

        let pixbuf_data = unsafe { pixbuf.get_pixels() };

        let mut surf = ImageSurface::create(cairo::Format::ARgb32, width, height)?;

        let width = width as usize;
        let height = height as usize;

        {
            let surf_stride = surf.get_stride() as usize;

            let mut surf_data = surf.get_data().unwrap();

            if has_alpha {
                for y in 0..height {
                    for x in 0..width {
                        let ofs = pixbuf_stride * y + 4 * x;

                        let pixel = Pixel {
                            r: pixbuf_data[ofs],
                            g: pixbuf_data[ofs + 1],
                            b: pixbuf_data[ofs + 2],
                            a: pixbuf_data[ofs + 3],
                        };

                        let pixel = pixel.premultiply();

                        surf_data.set_pixel(surf_stride, pixel, x as u32, y as u32);
                    }
                }
            } else {
                for y in 0..height {
                    for x in 0..width {
                        let ofs = pixbuf_stride * y + 3 * x;

                        let pixel = Pixel {
                            r: pixbuf_data[ofs],
                            g: pixbuf_data[ofs + 1],
                            b: pixbuf_data[ofs + 2],
                            a: 0xff,
                        };

                        surf_data.set_pixel(surf_stride, pixel, x as u32, y as u32);
                    }
                }
            }
        }

        Self::new(surf, SurfaceType::SRgb)
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

    /// Returns the surface stride.
    #[inline]
    pub fn stride(&self) -> isize {
        self.stride
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

        Pixel::from_u32(value)
    }

    /// Retrieves the pixel value by offset into the pixel data array.
    #[inline]
    pub fn get_pixel_by_offset(&self, offset: isize) -> Pixel {
        assert!(offset < self.stride as isize * self.height as isize);

        let value = unsafe { *(self.data_ptr.as_ptr().offset(offset) as *const u32) };
        Pixel::from_u32(value)
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
    pub fn convolve<R: Dim, C: Dim, S: Storage<f64, R, C>>(
        &self,
        bounds: IRect,
        target: (i32, i32),
        kernel: &Matrix<f64, R, C, S>,
        edge_mode: EdgeMode,
    ) -> Result<SharedImageSurface, cairo::Status> {
        assert!(kernel.nrows() >= 1);
        assert!(kernel.ncols() >= 1);

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
                        x1: x as i32 - target.0 + kernel.ncols() as i32,
                        y1: y as i32 - target.1 + kernel.nrows() as i32,
                    };

                    let mut a = 0.0;

                    for (x, y, pixel) in PixelRectangle::new(self, bounds, kernel_bounds, edge_mode)
                    {
                        let kernel_x = (kernel_bounds.x1 - x - 1) as usize;
                        let kernel_y = (kernel_bounds.y1 - y - 1) as usize;
                        let factor = kernel[(kernel_y, kernel_x)];

                        a += f64::from(pixel.a) * factor;
                    }

                    let convert = |x: f64| (clamp(x, 0.0, 255.0) + 0.5) as u8;

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
                        x1: x as i32 - target.0 + kernel.ncols() as i32,
                        y1: y as i32 - target.1 + kernel.nrows() as i32,
                    };

                    let mut r = 0.0;
                    let mut g = 0.0;
                    let mut b = 0.0;
                    let mut a = 0.0;

                    for (x, y, pixel) in PixelRectangle::new(self, bounds, kernel_bounds, edge_mode)
                    {
                        let kernel_x = (kernel_bounds.x1 - x - 1) as usize;
                        let kernel_y = (kernel_bounds.y1 - y - 1) as usize;
                        let factor = kernel[(kernel_y, kernel_x)];

                        r += f64::from(pixel.r) * factor;
                        g += f64::from(pixel.g) * factor;
                        b += f64::from(pixel.b) * factor;
                        a += f64::from(pixel.a) * factor;
                    }

                    let convert = |x: f64| (clamp(x, 0.0, 255.0) + 0.5) as u8;

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
    pub fn box_blur_loop<B: BlurDirection, A: IsAlphaOnly>(
        &self,
        output_surface: &mut cairo::ImageSurface,
        bounds: IRect,
        kernel_size: usize,
        target: usize,
    ) {
        assert_ne!(kernel_size, 0);
        assert!(target < kernel_size);
        assert_eq!(self.is_alpha_only(), A::IS_ALPHA_ONLY);

        {
            // The following code is needed for a parallel implementation of the blur loop. The
            // blurring is done either for each row or for each column of pixels, depending on the
            // value of `vertical`, independently of the others. Naturally, we want to run the
            // outer loop on a thread pool.
            //
            // The case of `vertical == false` is simple since the input image slice can be
            // partitioned into chunks for each row of pixels and processed in parallel with rayon.
            // The case of `vertical == true`, however, is more involved because we can't just make
            // mutable slices for all pixel columns (they would be overlapping which is forbidden
            // by the aliasing rules).
            //
            // This is where the following struct comes into play: it stores a sub-slice of the
            // pixel data and can be split at any row or column into two parts (similar to
            // slice::split_at_mut()).
            struct UnsafeSendPixelData<'a> {
                width: u32,
                height: u32,
                stride: isize,
                ptr: NonNull<u8>,
                _marker: PhantomData<&'a mut ()>,
            }

            unsafe impl<'a> Send for UnsafeSendPixelData<'a> {}

            impl<'a> UnsafeSendPixelData<'a> {
                /// Creates a new `UnsafeSendPixelData`.
                ///
                /// # Safety
                /// You must call `cairo_surface_mark_dirty()` on the surface once all instances of
                /// `UnsafeSendPixelData` are dropped to make sure the pixel changes are committed
                /// to Cairo.
                #[inline]
                unsafe fn new(surface: &mut cairo::ImageSurface) -> Self {
                    assert_eq!(surface.get_format(), cairo::Format::ARgb32);
                    let ptr = surface.get_data().unwrap().as_mut_ptr();

                    Self {
                        width: surface.get_width() as u32,
                        height: surface.get_height() as u32,
                        stride: surface.get_stride() as isize,
                        ptr: NonNull::new(ptr).unwrap(),
                        _marker: PhantomData,
                    }
                }

                /// Sets a pixel value at the given coordinates.
                #[inline]
                fn set_pixel(&mut self, pixel: Pixel, x: u32, y: u32) {
                    assert!(x < self.width);
                    assert!(y < self.height);

                    let value = pixel.to_u32();

                    unsafe {
                        let ptr = self
                            .ptr
                            .as_ptr()
                            .offset(y as isize * self.stride + x as isize * 4)
                            as *mut u32;
                        *ptr = value;
                    }
                }

                /// Splits this `UnsafeSendPixelData` into two at the given row.
                ///
                /// The first one contains rows `0..index` (`index` not included) and the second one
                /// contains rows `index..height`.
                #[inline]
                fn split_at_row(self, index: u32) -> (Self, Self) {
                    assert!(index <= self.height);

                    (
                        UnsafeSendPixelData {
                            width: self.width,
                            height: index,
                            stride: self.stride,
                            ptr: self.ptr,
                            _marker: PhantomData,
                        },
                        UnsafeSendPixelData {
                            width: self.width,
                            height: self.height - index,
                            stride: self.stride,
                            ptr: NonNull::new(unsafe {
                                self.ptr.as_ptr().offset(index as isize * self.stride)
                            })
                            .unwrap(),
                            _marker: PhantomData,
                        },
                    )
                }

                /// Splits this `UnsafeSendPixelData` into two at the given column.
                ///
                /// The first one contains columns `0..index` (`index` not included) and the second
                /// one contains columns `index..width`.
                #[inline]
                fn split_at_column(self, index: u32) -> (Self, Self) {
                    assert!(index <= self.width);

                    (
                        UnsafeSendPixelData {
                            width: index,
                            height: self.height,
                            stride: self.stride,
                            ptr: self.ptr,
                            _marker: PhantomData,
                        },
                        UnsafeSendPixelData {
                            width: self.width - index,
                            height: self.height,
                            stride: self.stride,
                            ptr: NonNull::new(unsafe {
                                self.ptr.as_ptr().offset(index as isize * 4)
                            })
                            .unwrap(),
                            _marker: PhantomData,
                        },
                    )
                }
            }

            let output_data = unsafe { UnsafeSendPixelData::new(output_surface) };

            // Shift is target into the opposite direction.
            let shift = (kernel_size - target) as i32;
            let target = target as i32;

            // Convert to f64 once since we divide by it.
            let kernel_size_f64 = kernel_size as f64;
            let compute = |x: u32| (f64::from(x) / kernel_size_f64 + 0.5) as u8;

            // Depending on `vertical`, we're blurring either horizontally line-by-line, or
            // vertically column-by-column. In the code below, the main axis is the axis along
            // which the blurring happens (so if `vertical` is false, the main axis is the
            // horizontal axis). The other axis is the outer loop axis. The code uses `i` and `j`
            // for the other axis and main axis coordinates, respectively.
            let (main_axis_min, main_axis_max, other_axis_min, other_axis_max) = if B::IS_VERTICAL {
                (bounds.y0, bounds.y1, bounds.x0, bounds.x1)
            } else {
                (bounds.x0, bounds.x1, bounds.y0, bounds.y1)
            };

            // Helper function for getting the pixels.
            let pixel = |i, j| {
                let (x, y) = if B::IS_VERTICAL { (i, j) } else { (j, i) };

                self.get_pixel(x as u32, y as u32)
            };

            // The following loop assumes the first row or column of `output_data` is the first row
            // or column inside `bounds`.
            let mut output_data = if B::IS_VERTICAL {
                output_data.split_at_column(bounds.x0 as u32).1
            } else {
                output_data.split_at_row(bounds.y0 as u32).1
            };

            rayon::scope(|s| {
                for i in other_axis_min..other_axis_max {
                    // Split off one row or column and launch its processing on another thread.
                    // Thanks to the initial split before the loop, there's no special case for the
                    // very first split.
                    let (mut current, remaining) = if B::IS_VERTICAL {
                        output_data.split_at_column(1)
                    } else {
                        output_data.split_at_row(1)
                    };

                    output_data = remaining;

                    s.spawn(move |_| {
                        // Helper function for setting the pixels.
                        let mut set_pixel = |j, pixel| {
                            // We're processing rows or columns one-by-one, so the other coordinate
                            // is always 0.
                            let (x, y) = if B::IS_VERTICAL { (0, j) } else { (j, 0) };
                            current.set_pixel(pixel, x, y);
                        };

                        // The idea is that since all weights of the box blur kernel are equal, for
                        // each step along the main axis, instead of recomputing the full sum, we
                        // can take the previous sum, subtract the "oldest" pixel value and add the
                        // "newest" pixel value.
                        //
                        // The sum is u32 so that it can fit MAXIMUM_KERNEL_SIZE * 255.
                        let mut sum_r = 0;
                        let mut sum_g = 0;
                        let mut sum_b = 0;
                        let mut sum_a = 0;

                        // The whole sum needs to be computed for the first pixel. However, we know
                        // that values outside of bounds are transparent, so the loop starts on the
                        // first pixel in bounds.
                        for j in main_axis_min..min(main_axis_max, main_axis_min + shift) {
                            let Pixel { r, g, b, a } = pixel(i, j);

                            if !A::IS_ALPHA_ONLY {
                                sum_r += u32::from(r);
                                sum_g += u32::from(g);
                                sum_b += u32::from(b);
                            }

                            sum_a += u32::from(a);
                        }

                        set_pixel(
                            main_axis_min as u32,
                            Pixel {
                                r: compute(sum_r),
                                g: compute(sum_g),
                                b: compute(sum_b),
                                a: compute(sum_a),
                            },
                        );

                        // Now, go through all the other pixels.
                        //
                        // j - target - 1 >= main_axis_min
                        // j >= main_axis_min + target + 1
                        let start_subtracting_at = main_axis_min + target + 1;

                        // j + shift - 1 < main_axis_max
                        // j < main_axis_max - shift + 1
                        let stop_adding_at = main_axis_max - shift + 1;

                        for j in main_axis_min + 1..main_axis_max {
                            if j >= start_subtracting_at {
                                let old_pixel = pixel(i, j - target - 1);

                                if !A::IS_ALPHA_ONLY {
                                    sum_r -= u32::from(old_pixel.r);
                                    sum_g -= u32::from(old_pixel.g);
                                    sum_b -= u32::from(old_pixel.b);
                                }

                                sum_a -= u32::from(old_pixel.a);
                            }

                            if j < stop_adding_at {
                                let new_pixel = pixel(i, j + shift - 1);

                                if !A::IS_ALPHA_ONLY {
                                    sum_r += u32::from(new_pixel.r);
                                    sum_g += u32::from(new_pixel.g);
                                    sum_b += u32::from(new_pixel.b);
                                }

                                sum_a += u32::from(new_pixel.a);
                            }

                            set_pixel(
                                j as u32,
                                Pixel {
                                    r: compute(sum_r),
                                    g: compute(sum_g),
                                    b: compute(sum_b),
                                    a: compute(sum_a),
                                },
                            );
                        }
                    });
                }
            });
        }

        // Don't forget to manually mark the surface as dirty (due to usage of
        // `UnsafeSendPixelData`).
        unsafe { cairo_sys::cairo_surface_mark_dirty(output_surface.to_raw_none()) }
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
    pub fn box_blur<B: BlurDirection>(
        &self,
        bounds: IRect,
        kernel_size: usize,
        target: usize,
    ) -> Result<SharedImageSurface, cairo::Status> {
        let mut output_surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)?;

        if self.is_alpha_only() {
            self.box_blur_loop::<B, AlphaOnly>(&mut output_surface, bounds, kernel_size, target);
        } else {
            self.box_blur_loop::<B, NotAlphaOnly>(&mut output_surface, bounds, kernel_size, target);
        }

        SharedImageSurface::new(output_surface, self.surface_type)
    }

    /// Returns a raw pointer to the underlying surface.
    ///
    /// # Safety
    /// The returned pointer must not be used to modify the surface.
    #[inline]
    pub unsafe fn to_glib_none(&self) -> Stash<'_, *mut cairo_sys::cairo_surface_t, ImageSurface> {
        self.surface.to_glib_none()
    }
}
