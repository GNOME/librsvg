use cairo;

use crate::float_eq_cairo::ApproxEqCairo;

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

pub trait RectangleExt {
    fn new(x: f64, y: f64, width: f64, height: f64) -> cairo::Rectangle;
    fn from_size(width: f64, height: f64) -> cairo::Rectangle;
    fn from_extents(x0: f64, y0: f64, x1: f64, y1: f64) -> cairo::Rectangle;
    fn is_empty(&self) -> bool;
    fn intersection(&self, rect: &cairo::Rectangle) -> Option<cairo::Rectangle>;
    fn union(&self, rect: &cairo::Rectangle) -> cairo::Rectangle;
    fn translate(&self, by: (f64, f64)) -> cairo::Rectangle;
}

impl RectangleExt for cairo::Rectangle {
    fn new(x: f64, y: f64, width: f64, height: f64) -> cairo::Rectangle {
        cairo::Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    fn from_size(width: f64, height: f64) -> cairo::Rectangle {
        cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    fn from_extents(x0: f64, y0: f64, x1: f64, y1: f64) -> cairo::Rectangle {
        cairo::Rectangle {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    fn is_empty(&self) -> bool {
        self.width.approx_eq_cairo(0.0) || self.height.approx_eq_cairo(0.0)
    }

    fn intersection(&self, rect: &cairo::Rectangle) -> Option<cairo::Rectangle> {
        let (x1, y1, x2, y2) = (
            self.x.max(rect.x),
            self.y.max(rect.y),
            (self.x + self.width).min(rect.x + rect.width),
            (self.y + self.height).min(rect.y + rect.height),
        );

        if x2 > x1 && y2 > y1 {
            Some(cairo::Rectangle {
                x: x1,
                y: y1,
                width: x2 - x1,
                height: y2 - y1,
            })
        } else {
            None
        }
    }

    fn union(&self, rect: &cairo::Rectangle) -> cairo::Rectangle {
        let (x1, y1, x2, y2) = (
            self.x.min(rect.x),
            self.y.min(rect.y),
            (self.x + self.width).max(rect.x + rect.width),
            (self.y + self.height).max(rect.y + rect.height),
        );

        cairo::Rectangle {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        }
    }

    fn translate(&self, by: (f64, f64)) -> cairo::Rectangle {
        cairo::Rectangle {
            x: self.x + by.0,
            y: self.y + by.1,
            width: self.width,
            height: self.height,
        }
    }
}

pub trait TransformRect {
    fn transform_rect(&self, rect: &cairo::Rectangle) -> cairo::Rectangle;
}

impl TransformRect for cairo::Matrix {
    fn transform_rect(&self, rect: &cairo::Rectangle) -> cairo::Rectangle {
        let points = vec![
            self.transform_point(rect.x, rect.y),
            self.transform_point(rect.x + rect.width, rect.y),
            self.transform_point(rect.x, rect.y + rect.height),
            self.transform_point(rect.x + rect.width, rect.y + rect.height),
        ];

        let (mut xmin, mut ymin, mut xmax, mut ymax) = {
            let (x, y) = points[0];

            (x, y, x, y)
        };

        for &(x, y) in points.iter().take(4).skip(1) {
            if x < xmin {
                xmin = x;
            }

            if x > xmax {
                xmax = x;
            }

            if y < ymin {
                ymin = y;
            }

            if y > ymax {
                ymax = y;
            }
        }

        cairo::Rectangle {
            x: xmin,
            y: ymin,
            width: xmax - xmin,
            height: ymax - ymin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rect() {
        let empty = cairo::Rectangle {
            x: 0.42,
            y: 0.42,
            width: 0.0,
            height: 0.0,
        };
        let not_empty = cairo::Rectangle {
            x: 0.22,
            y: 0.22,
            width: 3.14,
            height: 3.14,
        };

        assert!(empty.is_empty());
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn intersect_rects() {
        let r1 = cairo::Rectangle {
            x: 0.42,
            y: 0.42,
            width: 4.14,
            height: 4.14,
        };
        let r2 = cairo::Rectangle {
            x: 0.22,
            y: 0.22,
            width: 3.14,
            height: 3.14,
        };
        let r3 = cairo::Rectangle {
            x: 10.0,
            y: 10.0,
            width: 3.14,
            height: 3.14,
        };

        let r = r1.intersection(&r2).unwrap();
        assert_approx_eq_cairo!(0.42_f64, r.x);
        assert_approx_eq_cairo!(0.42_f64, r.y);
        assert_approx_eq_cairo!(2.94_f64, r.width);
        assert_approx_eq_cairo!(2.94_f64, r.height);

        let r = r1.intersection(&r3);
        assert!(r.is_none());
    }

    #[test]
    fn union_rects() {
        let r1 = cairo::Rectangle {
            x: 0.42,
            y: 0.42,
            width: 4.14,
            height: 4.14,
        };
        let r2 = cairo::Rectangle {
            x: 0.22,
            y: 0.22,
            width: 3.14,
            height: 3.14,
        };

        let r = r1.union(&r2);
        assert_approx_eq_cairo!(0.22_f64, r.x);
        assert_approx_eq_cairo!(0.22_f64, r.y);
        assert_approx_eq_cairo!(4.34_f64, r.width);
        assert_approx_eq_cairo!(4.34_f64, r.height);
    }

    #[test]
    fn transform_rect() {
        let r = cairo::Rectangle {
            x: 0.42,
            y: 0.42,
            width: 3.14,
            height: 3.14,
        };

        let m = cairo::Matrix::identity();
        let tr = m.transform_rect(&r);
        assert_eq!(tr, r);

        let m = cairo::Matrix::new(2.0, 0.0, 0.0, 2.0, 1.5, 1.5);
        let tr = m.transform_rect(&r);
        assert_approx_eq_cairo!(2.34_f64, tr.x);
        assert_approx_eq_cairo!(2.34_f64, tr.y);
        assert_approx_eq_cairo!(6.28_f64, tr.width);
        assert_approx_eq_cairo!(6.28_f64, tr.height);
    }
}
