use cairo;
use core::ops::Range;

use crate::float_eq_cairo::ApproxEqCairo;

pub trait RectangleExt {
    fn new(x: f64, y: f64, width: f64, height: f64) -> cairo::Rectangle;
    fn from_size(width: f64, height: f64) -> cairo::Rectangle;
    fn is_empty(&self) -> bool;
    fn intersection(&self, rect: &cairo::Rectangle) -> Option<cairo::Rectangle>;
    fn union(&self, rect: &cairo::Rectangle) -> cairo::Rectangle;
    fn transform(&self, affine: &cairo::Matrix) -> cairo::Rectangle;
    fn translate(&self, by: (f64, f64)) -> cairo::Rectangle;
    fn outer(&self) -> cairo::Rectangle;
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

    fn transform(&self, affine: &cairo::Matrix) -> cairo::Rectangle {
        let points = vec![
            affine.transform_point(self.x, self.y),
            affine.transform_point(self.x + self.width, self.y),
            affine.transform_point(self.x, self.y + self.height),
            affine.transform_point(self.x + self.width, self.y + self.height),
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

    fn translate(&self, by: (f64, f64)) -> cairo::Rectangle {
        cairo::Rectangle {
            x: self.x + by.0,
            y: self.y + by.1,
            width: self.width,
            height: self.height,
        }
    }

    fn outer(&self) -> cairo::Rectangle {
        let (x, y) = (self.x.floor(), self.y.floor());

        cairo::Rectangle {
            x,
            y,
            width: (self.x + self.width).ceil() - x,
            height: (self.y + self.height).ceil() - y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    #[inline]
    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> IRect {
        IRect { x0, y0, x1, y1 }
    }

    #[inline]
    pub fn from_size(w: i32, h: i32) -> IRect {
        IRect {
            x0: 0,
            y0: 0,
            x1: w,
            y1: h,
        }
    }

    #[inline]
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    #[inline]
    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }

    #[inline]
    pub fn x_range(&self) -> Range<i32> {
        self.x0..self.x1
    }

    #[inline]
    pub fn y_range(&self) -> Range<i32> {
        self.y0..self.y1
    }

    /// Returns true if the `IRect` contains the given coordinates.
    #[inline]
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    /// Returns an `IRect` scaled by the given amounts.
    ///
    /// The returned `IRect` encompasses all, even partially covered, pixels after the scaling.
    #[inline]
    pub fn scale(self, x: f64, y: f64) -> IRect {
        IRect {
            x0: (f64::from(self.x0) * x).floor() as i32,
            y0: (f64::from(self.y0) * y).floor() as i32,
            x1: (f64::from(self.x1) * x).ceil() as i32,
            y1: (f64::from(self.y1) * y).ceil() as i32,
        }
    }

    /// Returns an `IRect` translated by the given amounts.
    #[inline]
    pub fn translate(&self, by: (i32, i32)) -> IRect {
        IRect {
            x0: self.x0 + by.0,
            y0: self.y0 + by.1,
            x1: self.x1 + by.0,
            y1: self.y1 + by.1,
        }
    }

    #[inline]
    pub fn intersection(&self, rect: &Self) -> Option<IRect> {
        let (x0, y0, x1, y1) = (
            self.x0.max(rect.x0),
            self.y0.max(rect.y0),
            self.x1.min(rect.x1),
            self.y1.min(rect.y1),
        );

        if x1 > x0 && y1 > y0 {
            Some(IRect { x0, y0, x1, y1 })
        } else {
            None
        }
    }
}

impl From<cairo::Rectangle> for IRect {
    #[inline]
    fn from(
        cairo::Rectangle {
            x,
            y,
            width,
            height,
        }: cairo::Rectangle,
    ) -> Self {
        Self {
            x0: x.floor() as i32,
            y0: y.floor() as i32,
            x1: (x + width).ceil() as i32,
            y1: (y + height).ceil() as i32,
        }
    }
}

impl From<IRect> for cairo::Rectangle {
    #[inline]
    fn from(IRect { x0, y0, x1, y1 }: IRect) -> Self {
        Self {
            x: f64::from(x0),
            y: f64::from(y0),
            width: f64::from(x1 - x0),
            height: f64::from(y1 - y0),
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
        let tr = r.transform(&m);
        assert_eq!(tr, r);

        let m = cairo::Matrix::new(2.0, 0.0, 0.0, 2.0, 1.5, 1.5);
        let tr = r.transform(&m);
        assert_approx_eq_cairo!(2.34_f64, tr.x);
        assert_approx_eq_cairo!(2.34_f64, tr.y);
        assert_approx_eq_cairo!(6.28_f64, tr.width);
        assert_approx_eq_cairo!(6.28_f64, tr.height);
    }

    #[test]
    fn outer_rect() {
        let r = cairo::Rectangle {
            x: 1.42,
            y: 1.42,
            width: 3.14,
            height: 3.14,
        };

        let or = r.outer();
        assert_eq!(1.0, or.x);
        assert_eq!(1.0, or.y);
        assert_eq!(4.0, or.width);
        assert_eq!(4.0, or.height);
    }
}
