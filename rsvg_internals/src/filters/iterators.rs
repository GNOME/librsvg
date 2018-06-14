//! Pixel iterators for surfaces.
use std::ops::DerefMut;
use std::slice;

use cairo;
use cairo::prelude::SurfaceExt;
use cairo_sys;

use super::context::IRect;
use super::FilterError;

/// Shared (read-only) `cairo::ImageSurfaceData`.
// TODO: add to cairo itself?
#[derive(Debug, Clone, Copy)]
pub struct ImageSurfaceDataShared<'a> {
    data: &'a [u8],
    pub width: usize,
    pub height: usize,
    stride: usize,
}

/// A pixel consisting of R, G, B and A values.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Iterator over pixels of an image surface.
#[derive(Debug, Clone, Copy)]
pub struct Pixels<'a> {
    data: ImageSurfaceDataShared<'a>,
    bounds: IRect,
    x: usize,
    y: usize,
}

/// Extension methods for `cairo::ImageSurfaceData`.
pub trait ImageSurfaceDataExt: DerefMut<Target = [u8]> {
    /// Sets the pixel at the given coordinates.
    #[inline]
    fn set_pixel(&mut self, stride: usize, pixel: Pixel, x: usize, y: usize) {
        let value = ((pixel.a as u32) << 24)
            | ((pixel.r as u32) << 16)
            | ((pixel.g as u32) << 8)
            | (pixel.b as u32);
        unsafe {
            *(&mut self[y * stride + x * 4] as *mut u8 as *mut u32) = value;
        }
    }
}

impl<'a> ImageSurfaceDataShared<'a> {
    /// Creates a shared (read-only) surface data accessor for an image surface.
    ///
    /// # Safety
    /// The surface data must not be mutably aliased or modified for the lifetime of the returned
    /// data slice (in general it's `'a`, but usually it's restricted to the lifetime of the
    /// returned struct).
    ///
    /// This is an issue for this method too, because it's possible to modify the surface pixel
    /// data via a Cairo context, since it doesn't borrow the surface mutably.
    ///
    /// # Panics
    /// Panics if the surface isn't borrowed exclusively.
    #[inline]
    pub unsafe fn new(surface: &cairo::ImageSurface) -> Result<Self, FilterError> {
        Self::new_internal(surface, true)
    }

    /// Creates a shared (read-only) surface data accessor for an image surface without checking
    /// for surface exclusiveness.
    ///
    /// # Safety
    /// The surface data must not be mutably aliased or modified for the lifetime of the returned
    /// data slice (in general it's `'a`, but usually it's restricted to the lifetime of the
    /// returned struct).
    #[inline]
    pub unsafe fn new_unchecked(surface: &cairo::ImageSurface) -> Result<Self, FilterError> {
        Self::new_internal(surface, false)
    }

    /// Creates a shared (read-only) surface data accessor for an image surface, optionally
    /// checking for exclusive access.
    ///
    /// # Safety
    /// If `check_exclusive_access` is `false`, the surface data must not be mutably aliased for
    /// the lifetime of the returned data slice (in general it's `'a`, but usually it's restricted
    /// to the lifetime of the returned struct).
    unsafe fn new_internal(
        surface: &cairo::ImageSurface,
        check_exclusive_access: bool,
    ) -> Result<Self, FilterError> {
        if check_exclusive_access {
            assert!(cairo_sys::cairo_surface_get_reference_count(surface.to_raw_none()) == 1);
        }

        let width = surface.get_width() as usize;
        let height = surface.get_height() as usize;
        let stride = surface.get_stride() as usize;

        surface.flush();
        if surface.status() != cairo::Status::Success {
            return Err(FilterError::BadInputSurfaceStatus(surface.status()));
        }
        let data_ptr = cairo_sys::cairo_image_surface_get_data(surface.to_raw_none());
        assert!(!data_ptr.is_null());

        let data_len = stride * height;
        let data = slice::from_raw_parts(data_ptr, data_len);

        Ok(Self {
            data,
            width,
            height,
            stride,
        })
    }

    /// Retrieves the pixel value at the given coordinates.
    // Making this just #[inline] AND making Pixels::next() #[inline] prevents this from being
    // inlined in the benchmarks leading to significantly worse benchmark results. Making this
    // #[inline(always)] and making Pixels::new() #[inline] leads to good benchmark results.
    #[inline(always)]
    pub fn get_pixel(self, x: usize, y: usize) -> Pixel {
        assert!(x < self.width);
        assert!(y < self.height);

        let value = unsafe { *(&self.data[y * self.stride + x * 4] as *const u8 as *const u32) };

        Pixel {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
            a: ((value >> 24) & 0xFF) as u8,
        }
    }
}

impl<'a> Pixels<'a> {
    /// Creates an iterator over the image surface pixels, constrained within the given bounds.
    #[inline]
    pub fn new(data: ImageSurfaceDataShared<'a>, bounds: IRect) -> Self {
        // Sanity checks.
        assert!(bounds.x0 >= 0);
        assert!((bounds.x0 as usize) <= data.width);
        assert!(bounds.x1 >= bounds.x0);
        assert!((bounds.x1 as usize) <= data.width);
        assert!(bounds.y0 >= 0);
        assert!((bounds.y0 as usize) <= data.height);
        assert!(bounds.y1 >= bounds.y0);
        assert!((bounds.y1 as usize) <= data.height);

        Self {
            data,
            bounds,
            x: bounds.x0 as usize,
            y: bounds.y0 as usize,
        }
    }
}

impl<'a> Iterator for Pixels<'a> {
    type Item = (usize, usize, Pixel);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // This means we hit the end on the last iteration.
        if self.x == self.bounds.x1 as usize || self.y == self.bounds.y1 as usize {
            return None;
        }

        let rv = Some((self.x, self.y, self.data.get_pixel(self.x, self.y)));

        if self.x + 1 == self.bounds.x1 as usize {
            self.x = self.bounds.x0 as usize;
            self.y += 1;
        } else {
            self.x += 1;
        }

        rv
    }
}

impl<'a> ImageSurfaceDataExt for cairo::ImageSurfaceData<'a> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn isds_panic_outside() {
        const WIDTH: usize = 32;
        const HEIGHT: usize = 64;
        const STRIDE: usize = 8;
        let arr = [0; (WIDTH + STRIDE) * HEIGHT];

        let data = ImageSurfaceDataShared {
            data: &arr,
            width: WIDTH,
            height: HEIGHT,
            stride: STRIDE,
        };

        data.get_pixel(WIDTH, HEIGHT);
    }

    #[test]
    fn pixels_count() {
        const WIDTH: usize = 32;
        const HEIGHT: usize = 64;
        const STRIDE: usize = 8;
        let arr = [0; (WIDTH + STRIDE) * HEIGHT];

        let data = ImageSurfaceDataShared {
            data: &arr,
            width: WIDTH,
            height: HEIGHT,
            stride: STRIDE,
        };

        // Full image.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: WIDTH as i32,
            y1: HEIGHT as i32,
        };
        assert_eq!(Pixels::new(data, bounds).count(), WIDTH * HEIGHT);

        // 1-wide column.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: HEIGHT as i32,
        };
        assert_eq!(Pixels::new(data, bounds).count(), HEIGHT);

        // 1-tall row.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: WIDTH as i32,
            y1: 1,
        };
        assert_eq!(Pixels::new(data, bounds).count(), WIDTH);

        // 1×1.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: 1,
        };
        assert_eq!(Pixels::new(data, bounds).count(), 1);

        // Nothing (x0 == x1).
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: HEIGHT as i32,
        };
        assert_eq!(Pixels::new(data, bounds).count(), 0);

        // Nothing (y0 == y1).
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: WIDTH as i32,
            y1: 0,
        };
        assert_eq!(Pixels::new(data, bounds).count(), 0);

        // Nothing (x0 == x1, y0 == y1).
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
        };
        assert_eq!(Pixels::new(data, bounds).count(), 0);
    }
}
