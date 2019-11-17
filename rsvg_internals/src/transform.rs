//! CSS transform values.

use std::f64::consts::*;

use cssparser::{Parser, Token};

use crate::error::*;
use crate::rect::Rect;
use crate::parsers::{optional_comma, Parse};

pub type Transform = cairo::Matrix;

// Extension trait to prepare the switch from cairo::Matrix to euclid
pub trait TransformExt
where
    Self: std::marker::Sized,
{
    fn row_major(m11: f64, m12: f64, m21: f64, m22: f64, m31: f64, m32: f64) -> Self;

    fn is_invertible(&self) -> bool;

    fn inverse(&self) -> Option<Self>;

    fn pre_transform(&self, mat: &Self) -> Self;

    fn transform_rect(&self, rect: &Rect) -> Rect;
}

impl TransformExt for Transform {
    fn row_major(m11: f64, m12: f64, m21: f64, m22: f64, m31: f64, m32: f64) -> Self {
        cairo::Matrix::new(m11, m12, m21, m22, m31, m32)
    }

    fn is_invertible(&self) -> bool {
        self.try_invert().is_ok()
    }

    fn inverse(&self) -> Option<Self> {
        self.try_invert().ok()
    }

    fn pre_transform(&self, mat: &Self) -> Self {
        cairo::Matrix::multiply(mat, self)
    }
    fn transform_rect(&self, rect: &Rect) -> Rect {
        let points = vec![
            self.transform_point(rect.x0, rect.y0),
            self.transform_point(rect.x1, rect.y0),
            self.transform_point(rect.x0, rect.y1),
            self.transform_point(rect.x1, rect.y1),
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

        Rect {
            x0: xmin,
            y0: ymin,
            x1: xmax,
            y1: ymax,
        }
    }
}

impl Parse for Transform {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
        let loc = parser.current_source_location();

        let t = parse_transform_list(parser)?;

        if t.is_invertible() {
            Ok(t)
        } else {
            Err(loc.new_custom_error(ValueErrorKind::Value(
                "invalid transformation matrix".to_string(),
            )))
        }
    }
}

// This parser is for the "transform" attribute in SVG.
// Its operataion and grammar are described here:
// https://www.w3.org/TR/SVG/coords.html#TransformAttribute

fn parse_transform_list<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
    let mut t = Transform::identity();

    loop {
        if parser.is_exhausted() {
            break;
        }

        let m = parse_transform_command(parser)?;
        t = t.pre_transform(&m);

        optional_comma(parser);
    }

    Ok(t)
}

fn parse_transform_command<'i>(
    parser: &mut Parser<'i, '_>,
) -> Result<Transform, CssParseError<'i>> {
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
) -> Result<Transform, CssParseError<'i>> {
    let loc = parser.current_source_location();

    match name {
        "matrix" => parse_matrix_args(parser),
        "translate" => parse_translate_args(parser),
        "scale" => parse_scale_args(parser),
        "rotate" => parse_rotate_args(parser),
        "skewX" => parse_skewx_args(parser),
        "skewY" => parse_skewy_args(parser),
        _ => Err(loc.new_custom_error(ValueErrorKind::parse_error(
            "expected matrix|translate|scale|rotate|skewX|skewY",
        ))),
    }
}

fn parse_matrix_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
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

        Ok(Transform::row_major(xx, yx, xy, yy, x0, y0))
    })
}

fn parse_translate_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
    parser.parse_nested_block(|p| {
        let tx = f64::parse(p)?;

        let ty = p
            .try_parse(|p| {
                optional_comma(p);
                f64::parse(p)
            })
            .unwrap_or(0.0);

        Ok(Transform::row_major(1.0, 0.0, 0.0, 1.0, tx, ty))
    })
}

fn parse_scale_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
    parser.parse_nested_block(|p| {
        let x = f64::parse(p)?;

        let y = p
            .try_parse(|p| {
                optional_comma(p);
                f64::parse(p)
            })
            .unwrap_or(x);

        Ok(Transform::row_major(x, 0.0, 0.0, y, 0.0, 0.0))
    })
}

fn parse_rotate_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
    parser.parse_nested_block(|p| {
        let angle = f64::parse(p)? * PI / 180.0;

        let (tx, ty) = p
            .try_parse(|p| -> Result<_, CssParseError> {
                optional_comma(p);
                let tx = f64::parse(p)?;

                optional_comma(p);
                let ty = f64::parse(p)?;

                Ok((tx, ty))
            })
            .unwrap_or((0.0, 0.0));

        let (s, c) = angle.sin_cos();

        Ok(Transform::row_major(1.0, 0.0, 0.0, 1.0, tx, ty)
            .pre_transform(&Transform::row_major(c, s, -s, c, 0.0, 0.0))
            .pre_transform(&Transform::row_major(1.0, 0.0, 0.0, 1.0, -tx, -ty)))
    })
}

fn parse_skewx_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
    parser.parse_nested_block(|p| {
        let a = f64::parse(p)? * PI / 180.0;
        Ok(Transform::row_major(1.0, 0.0, a.tan(), 1.0, 0.0, 0.0))
    })
}

fn parse_skewy_args<'i>(parser: &mut Parser<'i, '_>) -> Result<Transform, CssParseError<'i>> {
    parser.parse_nested_block(|p| {
        let a = f64::parse(p)? * PI / 180.0;
        Ok(Transform::row_major(1.0, a.tan(), 0.0, 1.0, 0.0, 0.0))
    })
}

#[cfg(test)]
fn make_rotation_matrix(angle_degrees: f64, tx: f64, ty: f64) -> Transform {
    let angle = angle_degrees * PI / 180.0;

    let mut t = Transform::row_major(1.0, 0.0, 0.0, 1.0, tx, ty);

    let mut r = Transform::identity();
    r.rotate(angle);

    t = t.pre_transform(&r);
    t.pre_transform(&Transform::row_major(1.0, 0.0, 0.0, 1.0, -tx, -ty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::ApproxEq;
    use std::f64;

    fn parse_transform(s: &str) -> Result<Transform, CssParseError> {
        Transform::parse_str(s)
    }

    fn assert_matrix_eq(a: &Transform, b: &Transform) {
        let epsilon = 8.0 * f64::EPSILON; // kind of arbitrary, but allow for some sloppiness

        assert!(a.xx.approx_eq(b.xx, (epsilon, 1)));
        assert!(a.yx.approx_eq(b.yx, (epsilon, 1)));
        assert!(a.xy.approx_eq(b.xy, (epsilon, 1)));
        assert!(a.yy.approx_eq(b.yy, (epsilon, 1)));
        assert!(a.x0.approx_eq(b.x0, (epsilon, 1)));
        assert!(a.y0.approx_eq(b.y0, (epsilon, 1)));
    }

    #[test]
    fn parses_valid_transform() {
        let t = Transform::row_major(1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = Transform::row_major(10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix(30.0, 10.0, 10.0);

        let a = t.pre_transform(&s);
        assert_matrix_eq(
            &parse_transform("translate(20, 30), scale (10) rotate (30 10 10)").unwrap(),
            &a.pre_transform(&r),
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
        assert_matrix_eq(
            &parse_transform("matrix (1 2 3 4 5 6)").unwrap(),
            &Transform::row_major(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
        );

        assert_matrix_eq(
            &parse_transform("matrix(1,2,3,4 5 6)").unwrap(),
            &Transform::row_major(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
        );

        assert_matrix_eq(
            &parse_transform("matrix (1,2.25,-3.25e2,4 5 6)").unwrap(),
            &Transform::row_major(1.0, 2.25, -325.0, 4.0, 5.0, 6.0),
        );
    }

    #[test]
    fn parses_translate() {
        assert_matrix_eq(
            &parse_transform("translate(-1 -2)").unwrap(),
            &Transform::row_major(1.0, 0.0, 0.0, 1.0, -1.0, -2.0),
        );

        assert_matrix_eq(
            &parse_transform("translate(-1, -2)").unwrap(),
            &Transform::row_major(1.0, 0.0, 0.0, 1.0, -1.0, -2.0),
        );

        assert_matrix_eq(
            &parse_transform("translate(-1)").unwrap(),
            &Transform::row_major(1.0, 0.0, 0.0, 1.0, -1.0, 0.0),
        );
    }

    #[test]
    fn parses_scale() {
        assert_matrix_eq(
            &parse_transform("scale (-1)").unwrap(),
            &Transform::row_major(-1.0, 0.0, 0.0, -1.0, 0.0, 0.0),
        );

        assert_matrix_eq(
            &parse_transform("scale(-1 -2)").unwrap(),
            &Transform::row_major(-1.0, 0.0, 0.0, -2.0, 0.0, 0.0),
        );

        assert_matrix_eq(
            &parse_transform("scale(-1, -2)").unwrap(),
            &Transform::row_major(-1.0, 0.0, 0.0, -2.0, 0.0, 0.0),
        );
    }

    #[test]
    fn parses_rotate() {
        assert_matrix_eq(
            &parse_transform("rotate (30)").unwrap(),
            &make_rotation_matrix(30.0, 0.0, 0.0),
        );
        assert_matrix_eq(
            &parse_transform("rotate (30,-1,-2)").unwrap(),
            &make_rotation_matrix(30.0, -1.0, -2.0),
        );
        assert_matrix_eq(
            &parse_transform("rotate(30, -1, -2)").unwrap(),
            &make_rotation_matrix(30.0, -1.0, -2.0),
        );
    }

    fn make_skew_x_matrix(angle_degrees: f64) -> Transform {
        let a = angle_degrees * PI / 180.0;
        Transform::row_major(1.0, 0.0, a.tan(), 1.0, 0.0, 0.0)
    }

    fn make_skew_y_matrix(angle_degrees: f64) -> Transform {
        let mut m = make_skew_x_matrix(angle_degrees);
        m.yx = m.xy;
        m.xy = 0.0;
        m
    }

    #[test]
    fn parses_skew_x() {
        assert_matrix_eq(
            &parse_transform("skewX (30)").unwrap(),
            &make_skew_x_matrix(30.0),
        );
    }

    #[test]
    fn parses_skew_y() {
        assert_matrix_eq(
            &parse_transform("skewY (30)").unwrap(),
            &make_skew_y_matrix(30.0),
        );
    }

    #[test]
    fn parses_transform_list() {
        let t = Transform::row_major(1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = Transform::row_major(10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix(30.0, 10.0, 10.0);

        assert_matrix_eq(
            &parse_transform("scale(10)rotate(30, 10, 10)").unwrap(),
            &s.pre_transform(&r),
        );

        assert_matrix_eq(
            &parse_transform("translate(20, 30), scale (10)").unwrap(),
            &t.pre_transform(&s),
        );

        let a = t.pre_transform(&s);
        assert_matrix_eq(
            &parse_transform("translate(20, 30), scale (10) rotate (30 10 10)").unwrap(),
            &a.pre_transform(&r),
        );
    }

    #[test]
    fn parses_empty() {
        assert_matrix_eq(&parse_transform("").unwrap(), &Transform::identity());
    }
}
