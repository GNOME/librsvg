//! Types for rectangles.

#[allow(clippy::module_inception)]
mod rect {
    use crate::float_eq_cairo::ApproxEqCairo;
    use core::ops::{Add, Range, Sub};
    use num_traits::Zero;

    // Use our own min() and max() that are acceptable for floating point

    fn min<T: PartialOrd>(x: T, y: T) -> T {
        if x <= y {
            x
        } else {
            y
        }
    }

    fn max<T: PartialOrd>(x: T, y: T) -> T {
        if x >= y {
            x
        } else {
            y
        }
    }

    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rect<T> {
        pub x0: T,
        pub y0: T,
        pub x1: T,
        pub y1: T,
    }

    impl<T> Rect<T> {
        #[inline]
        pub fn new(x0: T, y0: T, x1: T, y1: T) -> Self {
            Self { x0, y0, x1, y1 }
        }
    }

    impl<T> Rect<T>
    where
        T: Copy + PartialOrd + PartialEq + Add<T, Output = T> + Sub<T, Output = T> + Zero,
    {
        #[inline]
        pub fn from_size(w: T, h: T) -> Self {
            Self {
                x0: Zero::zero(),
                y0: Zero::zero(),
                x1: w,
                y1: h,
            }
        }

        #[inline]
        pub fn width(&self) -> T {
            self.x1 - self.x0
        }

        #[inline]
        pub fn height(&self) -> T {
            self.y1 - self.y0
        }

        #[inline]
        pub fn size(&self) -> (T, T) {
            (self.width(), self.height())
        }

        #[inline]
        pub fn x_range(&self) -> Range<T> {
            self.x0..self.x1
        }

        #[inline]
        pub fn y_range(&self) -> Range<T> {
            self.y0..self.y1
        }

        #[inline]
        pub fn contains(self, x: T, y: T) -> bool {
            x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
        }

        #[inline]
        pub fn translate(&self, by: (T, T)) -> Self {
            Self {
                x0: self.x0 + by.0,
                y0: self.y0 + by.1,
                x1: self.x1 + by.0,
                y1: self.y1 + by.1,
            }
        }

        #[inline]
        pub fn intersection(&self, rect: &Self) -> Option<Self> {
            let (x0, y0, x1, y1) = (
                max(self.x0, rect.x0),
                max(self.y0, rect.y0),
                min(self.x1, rect.x1),
                min(self.y1, rect.y1),
            );

            if x1 > x0 && y1 > y0 {
                Some(Self { x0, y0, x1, y1 })
            } else {
                None
            }
        }

        #[inline]
        pub fn union(&self, rect: &Self) -> Self {
            Self {
                x0: min(self.x0, rect.x0),
                y0: min(self.y0, rect.y0),
                x1: max(self.x1, rect.x1),
                y1: max(self.y1, rect.y1),
            }
        }
    }

    impl Rect<i32> {
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.width() == Zero::zero() || self.height() == Zero::zero()
        }

        #[inline]
        pub fn scale(self, x: f64, y: f64) -> Self {
            Self {
                x0: (f64::from(self.x0) * x).floor() as i32,
                y0: (f64::from(self.y0) * y).floor() as i32,
                x1: (f64::from(self.x1) * x).ceil() as i32,
                y1: (f64::from(self.y1) * y).ceil() as i32,
            }
        }
    }

    impl Rect<f64> {
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.width().approx_eq_cairo(0.0) || self.height().approx_eq_cairo(0.0)
        }

        #[inline]
        pub fn scale(self, x: f64, y: f64) -> Self {
            Self {
                x0: self.x0 * x,
                y0: self.y0 * y,
                x1: self.x1 * x,
                y1: self.y1 * y,
            }
        }
    }
}

pub type Rect = rect::Rect<f64>;

impl From<Rect> for IRect {
    #[inline]
    fn from(r: Rect) -> Self {
        Self {
            x0: r.x0.floor() as i32,
            y0: r.y0.floor() as i32,
            x1: r.x1.ceil() as i32,
            y1: r.y1.ceil() as i32,
        }
    }
}

impl From<cairo::Rectangle> for Rect {
    #[inline]
    fn from(r: cairo::Rectangle) -> Self {
        Self {
            x0: r.x,
            y0: r.y,
            x1: r.x + r.width,
            y1: r.y + r.height,
        }
    }
}

impl From<Rect> for cairo::Rectangle {
    #[inline]
    fn from(r: Rect) -> Self {
        Self {
            x: r.x0,
            y: r.y0,
            width: r.x1 - r.x0,
            height: r.y1 - r.y0,
        }
    }
}

pub type IRect = rect::Rect<i32>;

impl From<IRect> for Rect {
    #[inline]
    fn from(r: IRect) -> Self {
        Self {
            x0: f64::from(r.x0),
            y0: f64::from(r.y0),
            x1: f64::from(r.x1),
            y1: f64::from(r.y1),
        }
    }
}

impl From<cairo::Rectangle> for IRect {
    #[inline]
    fn from(r: cairo::Rectangle) -> Self {
        Self {
            x0: r.x.floor() as i32,
            y0: r.y.floor() as i32,
            x1: (r.x + r.width).ceil() as i32,
            y1: (r.y + r.height).ceil() as i32,
        }
    }
}

impl From<IRect> for cairo::Rectangle {
    #[inline]
    fn from(r: IRect) -> Self {
        Self {
            x: f64::from(r.x0),
            y: f64::from(r.y0),
            width: f64::from(r.x1 - r.x0),
            height: f64::from(r.y1 - r.y0),
        }
    }
}
