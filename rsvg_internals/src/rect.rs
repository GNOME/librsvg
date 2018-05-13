use cairo;
use cairo::MatrixTrait;

pub fn intersect(r1: &cairo::Rectangle, r2: &cairo::Rectangle) -> cairo::Rectangle {
    let (x1, y1, x2, y2) = (
        r1.x.max(r2.x),
        r1.y.max(r2.y),
        (r1.x + r1.width).min(r2.x + r2.width),
        (r1.y + r1.height).min(r2.y + r2.height),
    );

    cairo::Rectangle {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    }
}

pub fn union(r1: &cairo::Rectangle, r2: &cairo::Rectangle) -> cairo::Rectangle {
    let (x1, y1, x2, y2) = (
        r1.x.min(r2.x),
        r1.y.min(r2.y),
        (r1.x + r1.width).max(r2.x + r2.width),
        (r1.y + r1.height).max(r2.y + r2.height),
    );

    cairo::Rectangle {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    }
}

pub fn transform(affine: &cairo::Matrix, rect: &cairo::Rectangle) -> cairo::Rectangle {
    let points = vec![
        affine.transform_point(rect.x, rect.y),
        affine.transform_point(rect.x + rect.width, rect.y),
        affine.transform_point(rect.x, rect.y + rect.height),
        affine.transform_point(rect.x + rect.width, rect.y + rect.height),
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

#[cfg(test)]
mod tests {
    use super::*;
    use float_eq_cairo::ApproxEqCairo;

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

        let r = intersect(&r1, &r2);
        assert_approx_eq_cairo!(0.42_f64, r.x);
        assert_approx_eq_cairo!(0.42_f64, r.y);
        assert_approx_eq_cairo!(2.94_f64, r.width);
        assert_approx_eq_cairo!(2.94_f64, r.height);
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

        let r = union(&r1, &r2);
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
        let tr = transform(&m, &r);
        assert_eq!(tr, r);

        let m = cairo::Matrix::new(2.0, 0.0, 0.0, 2.0, 1.5, 1.5);
        let tr = transform(&m, &r);
        assert_approx_eq_cairo!(2.34_f64, tr.x);
        assert_approx_eq_cairo!(2.34_f64, tr.y);
        assert_approx_eq_cairo!(6.28_f64, tr.width);
        assert_approx_eq_cairo!(6.28_f64, tr.height);
    }
}
