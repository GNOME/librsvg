use cairo;
use cairo::MatrixTrait;

use float_eq_cairo::ApproxEqCairo;

pub trait RectangleExt {
    fn is_empty(&self) -> bool;
    fn intersect(&self, rect: &cairo::Rectangle) -> cairo::Rectangle;
    fn union(&self, rect: &cairo::Rectangle) -> cairo::Rectangle;
    fn transform(&self, affine: &cairo::Matrix) -> cairo::Rectangle;
    fn outer(&self) -> cairo::Rectangle;
}

impl RectangleExt for cairo::Rectangle {
    fn is_empty(&self) -> bool {
        self.width.approx_eq_cairo(&0.0) || self.height.approx_eq_cairo(&0.0)
    }

    fn intersect(&self, rect: &cairo::Rectangle) -> cairo::Rectangle {
        let (x1, y1, x2, y2) = (
            self.x.max(rect.x),
            self.y.max(rect.y),
            (self.x + self.width).min(rect.x + rect.width),
            (self.y + self.height).min(rect.y + rect.height),
        );

        if x2 > x1 && y2 > y1 {
            cairo::Rectangle {
                x: x1,
                y: y1,
                width: x2 - x1,
                height: y2 - y1,
            }
        } else {
            cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            }
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

        for i in 1..4 {
            let (x, y) = points[i];

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

        let r = r1.intersect(&r2);
        assert_approx_eq_cairo!(0.42_f64, r.x);
        assert_approx_eq_cairo!(0.42_f64, r.y);
        assert_approx_eq_cairo!(2.94_f64, r.width);
        assert_approx_eq_cairo!(2.94_f64, r.height);

        let r = r1.intersect(&r3);
        assert!(r.is_empty());
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
