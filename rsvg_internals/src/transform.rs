//! Handling of `transform` values.
//!
//! This module handles `transform` values [per the SVG specification][spec].
//!
//! [spec]:  https://www.w3.org/TR/SVG11/coords.html#TransformAttribute

use cssparser::{Parser, Token};

use crate::angle::Angle;
use crate::error::*;
use crate::parsers::{optional_comma, Parse};
use crate::rect::Rect;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Transform {
    pub xx: f64,
    pub yx: f64,
    pub xy: f64,
    pub yy: f64,
    pub x0: f64,
    pub y0: f64,
}

impl Transform {
    #[inline]
    pub fn new_unchecked(xx: f64, yx: f64, xy: f64, yy: f64, x0: f64, y0: f64) -> Self {
        Self {
            xx,
            xy,
            x0,
            yx,
            yy,
            y0,
        }
    }

    #[inline]
    pub fn identity() -> Self {
        Self::new_unchecked(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    }

    #[inline]
    pub fn new_translate(tx: f64, ty: f64) -> Self {
        Self::new_unchecked(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    #[inline]
    pub fn new_scale(sx: f64, sy: f64) -> Self {
        Self::new_unchecked(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    #[inline]
    pub fn new_rotate(a: Angle) -> Self {
        let (s, c) = a.radians().sin_cos();
        Self::new_unchecked(c, s, -s, c, 0.0, 0.0)
    }

    #[inline]
    pub fn new_skew(ax: Angle, ay: Angle) -> Self {
        Self::new_unchecked(1.0, ay.radians().tan(), ax.radians().tan(), 1.0, 0.0, 0.0)
    }

    #[must_use]
    pub fn multiply(t1: &Transform, t2: &Transform) -> Self {
        Transform {
            xx: t1.xx * t2.xx + t1.yx * t2.xy,
            yx: t1.xx * t2.yx + t1.yx * t2.yy,
            xy: t1.xy * t2.xx + t1.yy * t2.xy,
            yy: t1.xy * t2.yx + t1.yy * t2.yy,
            x0: t1.x0 * t2.xx + t1.y0 * t2.xy + t2.x0,
            y0: t1.x0 * t2.yx + t1.y0 * t2.yy + t2.y0,
        }
    }

    #[inline]
    pub fn pre_transform(&self, t: &Transform) -> Self {
        Self::multiply(t, self)
    }

    #[inline]
    pub fn post_transform(&self, t: &Transform) -> Self {
        Self::multiply(self, t)
    }

    #[inline]
    pub fn pre_translate(&self, x: f64, y: f64) -> Self {
        self.pre_transform(&Transform::new_translate(x, y))
    }

    #[inline]
    pub fn pre_scale(&self, sx: f64, sy: f64) -> Self {
        self.pre_transform(&Transform::new_scale(sx, sy))
    }

    #[inline]
    pub fn pre_rotate(&self, angle: Angle) -> Self {
        self.pre_transform(&Transform::new_rotate(angle))
    }

    #[inline]
    pub fn post_translate(&self, x: f64, y: f64) -> Self {
        self.post_transform(&Transform::new_translate(x, y))
    }

    #[inline]
    pub fn post_scale(&self, sx: f64, sy: f64) -> Self {
        self.post_transform(&Transform::new_scale(sx, sy))
    }

    #[inline]
    pub fn post_rotate(&self, angle: Angle) -> Self {
        self.post_transform(&Transform::new_rotate(angle))
    }

    #[inline]
    fn determinant(&self) -> f64 {
        self.xx * self.yy - self.xy * self.yx
    }

    #[inline]
    pub fn is_invertible(&self) -> bool {
        let det = self.determinant();

        det != 0.0 && det.is_finite()
    }

    #[must_use]
    pub fn invert(&self) -> Option<Self> {
        let det = self.determinant();

        if det == 0.0 || !det.is_finite() {
            return None;
        }

        let inv_det = 1.0 / det;

        Some(Transform::new_unchecked(
            inv_det * self.yy,
            inv_det * (-self.yx),
            inv_det * (-self.xy),
            inv_det * self.xx,
            inv_det * (self.xy * self.y0 - self.yy * self.x0),
            inv_det * (self.yx * self.x0 - self.xx * self.y0),
        ))
    }

    #[inline]
    pub fn transform_distance(&self, dx: f64, dy: f64) -> (f64, f64) {
        (dx * self.xx + dy * self.xy, dx * self.yx + dy * self.yy)
    }

    #[inline]
    pub fn transform_point(&self, px: f64, py: f64) -> (f64, f64) {
        let (x, y) = self.transform_distance(px, py);
        (x + self.x0, y + self.y0)
    }

    pub fn transform_rect(&self, rect: &Rect) -> Rect {
        let points = [
            self.transform_point(rect.x0, rect.y0),
            self.transform_point(rect.x1, rect.y0),
            self.transform_point(rect.x0, rect.y1),
            self.transform_point(rect.x1, rect.y1),
        ];

        let (mut xmin, mut ymin, mut xmax, mut ymax) = {
            let (x, y) = points[0];

            (x, y, x, y)
        };

        for &(x, y) in points.iter().skip(1) {
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

        Rect {
            x0: xmin,
            y0: ymin,
            x1: xmax,
            y1: ymax,
        }
    }
}

impl Default for Transform {
    #[inline]
    fn default() -> Transform {
        Transform::identity()
    }
}

impl Parse for Transform {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
        let loc = parser.current_source_location();

        let t = parse_transform_list(parser)?;

        if !t.is_invertible() {
            return Err(loc.new_custom_error(ValueErrorKind::Value(
                "invalid transformation matrix".to_string(),
            )));
        }

        Ok(t)
    }
}

impl From<cairo::Matrix> for Transform {
    #[inline]
    fn from(m: cairo::Matrix) -> Self {
        Self::new_unchecked(m.xx, m.yx, m.xy, m.yy, m.x0, m.y0)
    }
}

impl From<Transform> for cairo::Matrix {
    #[inline]
    fn from(t: Transform) -> Self {
        Self::new(t.xx, t.yx, t.xy, t.yy, t.x0, t.y0)
    }
}

fn parse_transform_list<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    let mut t = Transform::identity();

    loop {
        if parser.is_exhausted() {
            break;
        }

        t = parse_transform_command(parser)?.post_transform(&t);
        optional_comma(parser);
    }

    Ok(t)
}

fn parse_transform_command<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    let loc = parser.current_source_location();

    match parser.next()?.clone() {
        Token::Function(ref name) => parse_transform_function(name, parser),

        Token::Ident(ref name) => {
            parser.expect_parenthesis_block()?;
            parse_transform_function(name, parser)
        }

        tok => Err(loc.new_unexpected_token_error(tok.clone())),
    }
}

fn parse_transform_function<'i>(
    name: &str,
    parser: &mut Parser<'i, '_>,
) -> Result<Transform, ParseError<'i>> {
    let loc = parser.current_source_location();

    match name {
        "matrix" => parse_matrix_args(parser),
        "translate" => parse_translate_args(parser),
        "scale" => parse_scale_args(parser),
        "rotate" => parse_rotate_args(parser),
        "skewX" => parse_skew_x_args(parser),
        "skewY" => parse_skew_y_args(parser),
        _ => Err(loc.new_custom_error(ValueErrorKind::parse_error(
            "expected matrix|translate|scale|rotate|skewX|skewY",
        ))),
    }
}

fn parse_matrix_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    parser.parse_nested_block(|p| {
        let xx = f64::parse(p)?;
        optional_comma(p);

        let yx = f64::parse(p)?;
        optional_comma(p);

        let xy = f64::parse(p)?;
        optional_comma(p);

        let yy = f64::parse(p)?;
        optional_comma(p);

        let x0 = f64::parse(p)?;
        optional_comma(p);

        let y0 = f64::parse(p)?;

        Ok(Transform::new_unchecked(xx, yx, xy, yy, x0, y0))
    })
}

fn parse_translate_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    parser.parse_nested_block(|p| {
        let tx = f64::parse(p)?;

        let ty = p
            .try_parse(|p| {
                optional_comma(p);
                f64::parse(p)
            })
            .unwrap_or(0.0);

        Ok(Transform::new_translate(tx, ty))
    })
}

fn parse_scale_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    parser.parse_nested_block(|p| {
        let x = f64::parse(p)?;

        let y = p
            .try_parse(|p| {
                optional_comma(p);
                f64::parse(p)
            })
            .unwrap_or(x);

        Ok(Transform::new_scale(x, y))
    })
}

fn parse_rotate_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    parser.parse_nested_block(|p| {
        let angle = Angle::from_degrees(f64::parse(p)?);

        let (tx, ty) = p
            .try_parse(|p| -> Result<_, ParseError> {
                optional_comma(p);
                let tx = f64::parse(p)?;

                optional_comma(p);
                let ty = f64::parse(p)?;

                Ok((tx, ty))
            })
            .unwrap_or((0.0, 0.0));

        Ok(Transform::new_translate(tx, ty)
            .pre_rotate(angle)
            .pre_translate(-tx, -ty))
    })
}

fn parse_skew_x_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    parser.parse_nested_block(|p| {
        let angle = Angle::from_degrees(f64::parse(p)?);
        Ok(Transform::new_skew(angle, Angle::new(0.0)))
    })
}

fn parse_skew_y_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, ParseError<'i>> {
    parser.parse_nested_block(|p| {
        let angle = Angle::from_degrees(f64::parse(p)?);
        Ok(Transform::new_skew(Angle::new(0.0), angle))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::ApproxEq;
    use std::f64;

    fn rotation_transform(deg: f64, tx: f64, ty: f64) -> Transform {
        Transform::new_translate(tx, ty)
            .pre_rotate(Angle::from_degrees(deg))
            .pre_translate(-tx, -ty)
    }

    fn parse_transform(s: &str) -> Result<Transform, ParseError> {
        Transform::parse_str(s)
    }

    fn assert_transform_eq(t1: &Transform, t2: &Transform) {
        let epsilon = 8.0 * f64::EPSILON; // kind of arbitrary, but allow for some sloppiness

        assert!(t1.xx.approx_eq(t2.xx, (epsilon, 1)));
        assert!(t1.yx.approx_eq(t2.yx, (epsilon, 1)));
        assert!(t1.xy.approx_eq(t2.xy, (epsilon, 1)));
        assert!(t1.yy.approx_eq(t2.yy, (epsilon, 1)));
        assert!(t1.x0.approx_eq(t2.x0, (epsilon, 1)));
        assert!(t1.y0.approx_eq(t2.y0, (epsilon, 1)));
    }

    #[test]
    fn test_multiply() {
        let t1 = Transform::identity();
        let t2 = Transform::new_unchecked(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_transform_eq(&Transform::multiply(&t1, &t2), &t2);
        assert_transform_eq(&Transform::multiply(&t2, &t1), &t2);

        let t1 = Transform::new_unchecked(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let t2 = Transform::new_unchecked(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let r = Transform::new_unchecked(0.0, 0.0, 0.0, 0.0, 5.0, 6.0);
        assert_transform_eq(&Transform::multiply(&t1, &t2), &t2);
        assert_transform_eq(&Transform::multiply(&t2, &t1), &r);

        let t1 = Transform::new_unchecked(0.5, 0.0, 0.0, 0.5, 10.0, 10.0);
        let t2 = Transform::new_unchecked(1.0, 0.0, 0.0, 1.0, -10.0, -10.0);
        let r1 = Transform::new_unchecked(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        let r2 = Transform::new_unchecked(0.5, 0.0, 0.0, 0.5, 5.0, 5.0);
        assert_transform_eq(&Transform::multiply(&t1, &t2), &r1);
        assert_transform_eq(&Transform::multiply(&t2, &t1), &r2);
    }

    #[test]
    fn test_invert() {
        let t = Transform::new_unchecked(2.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(!t.is_invertible());
        assert!(t.invert().is_none());

        let t = Transform::identity();
        assert!(t.is_invertible());
        assert!(t.invert().is_some());
        let i = t.invert().unwrap();
        assert_transform_eq(&i, &Transform::identity());

        let t = Transform::new_unchecked(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert!(t.is_invertible());
        assert!(t.invert().is_some());
        let i = t.invert().unwrap();
        assert_transform_eq(&t.pre_transform(&i), &Transform::identity());
        assert_transform_eq(&t.post_transform(&i), &Transform::identity());
    }

    #[test]
    pub fn test_transform_point() {
        let t = Transform::new_translate(10.0, 10.0);
        assert_eq!((11.0, 11.0), t.transform_point(1.0, 1.0));
    }

    #[test]
    pub fn test_transform_distance() {
        let t = Transform::new_translate(10.0, 10.0).pre_scale(2.0, 1.0);
        assert_eq!((2.0, 1.0), t.transform_distance(1.0, 1.0));
    }

    #[test]
    fn parses_valid_transform() {
        let t = Transform::new_unchecked(1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = Transform::new_unchecked(10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = rotation_transform(30.0, 10.0, 10.0);

        let a = Transform::multiply(&s, &t);
        assert_transform_eq(
            &parse_transform("translate(20, 30), scale (10) rotate (30 10 10)").unwrap(),
            &Transform::multiply(&r, &a),
        );
    }

    fn assert_parse_error(s: &str) {
        assert!(parse_transform(s).is_err());
    }

    #[test]
    fn syntax_error_yields_parse_error() {
        assert_parse_error("foo");
        assert_parse_error("matrix (1 2 3 4 5)");
        assert_parse_error("translate(1 2 3 4 5)");
        assert_parse_error("translate (1,)");
        assert_parse_error("scale (1,)");
        assert_parse_error("skewX (1,2)");
        assert_parse_error("skewY ()");
        assert_parse_error("skewY");
    }

    #[test]
    fn invalid_transform_yields_value_error() {
        assert!(parse_transform("matrix (0 0 0 0 0 0)").is_err());
        assert!(parse_transform("scale (0), translate (10, 10)").is_err());
        assert!(parse_transform("scale (0), skewX (90)").is_err());
    }

    #[test]
    fn parses_matrix() {
        assert_transform_eq(
            &parse_transform("matrix (1 2 3 4 5 6)").unwrap(),
            &Transform::new_unchecked(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
        );

        assert_transform_eq(
            &parse_transform("matrix(1,2,3,4 5 6)").unwrap(),
            &Transform::new_unchecked(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
        );

        assert_transform_eq(
            &parse_transform("matrix (1,2.25,-3.25e2,4 5 6)").unwrap(),
            &Transform::new_unchecked(1.0, 2.25, -325.0, 4.0, 5.0, 6.0),
        );
    }

    #[test]
    fn parses_translate() {
        assert_transform_eq(
            &parse_transform("translate(-1 -2)").unwrap(),
            &Transform::new_unchecked(1.0, 0.0, 0.0, 1.0, -1.0, -2.0),
        );

        assert_transform_eq(
            &parse_transform("translate(-1, -2)").unwrap(),
            &Transform::new_unchecked(1.0, 0.0, 0.0, 1.0, -1.0, -2.0),
        );

        assert_transform_eq(
            &parse_transform("translate(-1)").unwrap(),
            &Transform::new_unchecked(1.0, 0.0, 0.0, 1.0, -1.0, 0.0),
        );
    }

    #[test]
    fn parses_scale() {
        assert_transform_eq(
            &parse_transform("scale (-1)").unwrap(),
            &Transform::new_unchecked(-1.0, 0.0, 0.0, -1.0, 0.0, 0.0),
        );

        assert_transform_eq(
            &parse_transform("scale(-1 -2)").unwrap(),
            &Transform::new_unchecked(-1.0, 0.0, 0.0, -2.0, 0.0, 0.0),
        );

        assert_transform_eq(
            &parse_transform("scale(-1, -2)").unwrap(),
            &Transform::new_unchecked(-1.0, 0.0, 0.0, -2.0, 0.0, 0.0),
        );
    }

    #[test]
    fn parses_rotate() {
        assert_transform_eq(
            &parse_transform("rotate (30)").unwrap(),
            &rotation_transform(30.0, 0.0, 0.0),
        );
        assert_transform_eq(
            &parse_transform("rotate (30,-1,-2)").unwrap(),
            &rotation_transform(30.0, -1.0, -2.0),
        );
        assert_transform_eq(
            &parse_transform("rotate(30, -1, -2)").unwrap(),
            &rotation_transform(30.0, -1.0, -2.0),
        );
    }

    #[test]
    fn parses_skew_x() {
        assert_transform_eq(
            &parse_transform("skewX (30)").unwrap(),
            &Transform::new_skew(Angle::from_degrees(30.0), Angle::new(0.0)),
        );
    }

    #[test]
    fn parses_skew_y() {
        assert_transform_eq(
            &parse_transform("skewY (30)").unwrap(),
            &Transform::new_skew(Angle::new(0.0), Angle::from_degrees(30.0)),
        );
    }

    #[test]
    fn parses_transform_list() {
        let t = Transform::new_unchecked(1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = Transform::new_unchecked(10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = rotation_transform(30.0, 10.0, 10.0);

        assert_transform_eq(
            &parse_transform("scale(10)rotate(30, 10, 10)").unwrap(),
            &Transform::multiply(&r, &s),
        );

        assert_transform_eq(
            &parse_transform("translate(20, 30), scale (10)").unwrap(),
            &Transform::multiply(&s, &t),
        );

        let a = Transform::multiply(&s, &t);
        assert_transform_eq(
            &parse_transform("translate(20, 30), scale (10) rotate (30 10 10)").unwrap(),
            &Transform::multiply(&r, &a),
        );
    }

    #[test]
    fn parses_empty() {
        assert_transform_eq(&parse_transform("").unwrap(), &Transform::identity());
    }
}
