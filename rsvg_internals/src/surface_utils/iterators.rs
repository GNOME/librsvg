//! Pixel iterators for `SharedImageSurface`.
use filters::context::IRect;

use super::shared_surface::SharedImageSurface;
use super::Pixel;

/// Iterator over pixels of a `SharedImageSurface`.
#[derive(Debug, Clone, Copy)]
pub struct Pixels<'a> {
    surface: &'a SharedImageSurface,
    bounds: IRect,
    x: u32,
    y: u32,
}

impl<'a> Pixels<'a> {
    /// Creates an iterator over the image surface pixels, constrained within the given bounds.
    #[inline]
    pub fn new(surface: &'a SharedImageSurface, bounds: IRect) -> Self {
        // Sanity checks.
        assert!(bounds.x0 >= 0);
        assert!(bounds.x0 <= surface.width());
        assert!(bounds.x1 >= bounds.x0);
        assert!(bounds.x1 <= surface.width());
        assert!(bounds.y0 >= 0);
        assert!(bounds.y0 <= surface.height());
        assert!(bounds.y1 >= bounds.y0);
        assert!(bounds.y1 <= surface.height());

        Self {
            surface,
            bounds,
            x: bounds.x0 as u32,
            y: bounds.y0 as u32,
        }
    }
}

impl<'a> Iterator for Pixels<'a> {
    type Item = (u32, u32, Pixel);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // This means we hit the end on the last iteration.
        if self.x == self.bounds.x1 as u32 || self.y == self.bounds.y1 as u32 {
            return None;
        }

        let rv = Some((self.x, self.y, self.surface.get_pixel(self.x, self.y)));

        if self.x + 1 == self.bounds.x1 as u32 {
            self.x = self.bounds.x0 as u32;
            self.y += 1;
        } else {
            self.x += 1;
        }

        rv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{self, ImageSurface};

    #[test]
    fn pixels_count() {
        const WIDTH: i32 = 32;
        const HEIGHT: i32 = 64;

        let surface = SharedImageSurface::new(
            ImageSurface::create(cairo::Format::ARgb32, WIDTH, HEIGHT).unwrap(),
        ).unwrap();

        // Full image.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: WIDTH,
            y1: HEIGHT,
        };
        assert_eq!(
            Pixels::new(&surface, bounds).count(),
            (WIDTH * HEIGHT) as usize
        );

        // 1-wide column.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: HEIGHT,
        };
        assert_eq!(Pixels::new(&surface, bounds).count(), HEIGHT as usize);

        // 1-tall row.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: WIDTH,
            y1: 1,
        };
        assert_eq!(Pixels::new(&surface, bounds).count(), WIDTH as usize);

        // 1×1.
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 1,
            y1: 1,
        };
        assert_eq!(Pixels::new(&surface, bounds).count(), 1);

        // Nothing (x0 == x1).
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: HEIGHT,
        };
        assert_eq!(Pixels::new(&surface, bounds).count(), 0);

        // Nothing (y0 == y1).
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: WIDTH,
            y1: 0,
        };
        assert_eq!(Pixels::new(&surface, bounds).count(), 0);

        // Nothing (x0 == x1, y0 == y1).
        let bounds = IRect {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
        };
        assert_eq!(Pixels::new(&surface, bounds).count(), 0);
    }
}
