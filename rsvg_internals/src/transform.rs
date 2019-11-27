use cairo;

use std::f64::consts::*;

use cssparser::{ParseError as CssParseError, Parser, Token};

use crate::error::*;
use crate::parsers::{finite_f32, CssParserExt, Parse, ParseError};

pub type Transform = cairo::Matrix;

// Extension trait to prepare the switch from cairo::Matrix to euclid
pub trait TransformExt
where Self: std::marker::Sized
{
    fn row_major(m11: f64, m12: f64, m21: f64, m22: f64, m31: f64, m32: f64) -> Self;

    fn inverse(&self) -> Option<Self>;

    fn pre_transform(&self, mat: &Self) -> Self;

    fn transform_rect(&self, rect: &cairo::Rectangle) -> cairo::Rectangle;
}

impl TransformExt for Transform {
    fn row_major(m11: f64, m12: f64, m21: f64, m22: f64, m31: f64, m32: f64) -> Self {
        cairo::Matrix::new(m11, m12, m21, m22, m31, m32)
    }

    fn inverse(&self) -> Option<Self> {
        self.try_invert().ok()
    }

    fn pre_transform(&self, mat: &Self) -> Self {
        cairo::Matrix::multiply(mat, self)
    }

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

impl Parse for Transform {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
        let matrix = parse_transform_list(parser)?;

        match matrix.inverse() {
            Some(_) => Ok(matrix),
            _ => Err(ValueErrorKind::Value("invalid transformation matrix".to_string())),
        }
    }
}

// This parser is for the "transform" attribute in SVG.
// Its operataion and grammar are described here:
// https://www.w3.org/TR/SVG/coords.html#TransformAttribute

fn parse_transform_list(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    let mut matrix = Transform::identity();

    loop {
        if parser.is_exhausted() {
            break;
        }

        let m = parse_transform_command(parser)?;
        matrix = matrix.pre_transform(&m);

        parser.optional_comma();
    }

    Ok(matrix)
}

fn make_expected_function_error() -> ValueErrorKind {
    ValueErrorKind::from(ParseError::new(
        "expected matrix|translate|scale|rotate|skewX|skewY",
    ))
}

fn parse_transform_command(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    match parser.next()?.clone() {
        Token::Function(ref name) => parse_transform_function(name, parser),

        Token::Ident(ref name) => {
            parser.expect_parenthesis_block()?;
            parse_transform_function(name, parser)
        }

        _ => Err(make_expected_function_error()),
    }
}

fn parse_transform_function(
    name: &str,
    parser: &mut Parser<'_, '_>,
) -> Result<Transform, ValueErrorKind> {
    match name {
        "matrix" => parse_matrix_args(parser),
        "translate" => parse_translate_args(parser),
        "scale" => parse_scale_args(parser),
        "rotate" => parse_rotate_args(parser),
        "skewX" => parse_skewx_args(parser),
        "skewY" => parse_skewy_args(parser),
        _ => Err(make_expected_function_error()),
    }
}

fn parse_matrix_args(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let xx = p.expect_number()?;
            p.optional_comma();

            let yx = p.expect_number()?;
            p.optional_comma();

            let xy = p.expect_number()?;
            p.optional_comma();

            let yy = p.expect_number()?;
            p.optional_comma();

            let x0 = p.expect_number()?;
            p.optional_comma();

            let y0 = p.expect_number()?;

            Ok((xx, yx, xy, yy, x0, y0))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(ValueErrorKind::from)
        .and_then(|(xx, yx, xy, yy, x0, y0)| {
            let xx = f64::from(finite_f32(xx)?);
            let yx = f64::from(finite_f32(yx)?);
            let xy = f64::from(finite_f32(xy)?);
            let yy = f64::from(finite_f32(yy)?);
            let x0 = f64::from(finite_f32(x0)?);
            let y0 = f64::from(finite_f32(y0)?);

            Ok(Transform::row_major(xx, yx, xy, yy, x0, y0))
        })
}

fn parse_translate_args(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let tx = p.expect_number()?;

            let ty = p
                .try_parse(|p| -> Result<f32, CssParseError<'_, ()>> {
                    p.optional_comma();
                    Ok(p.expect_number()?)
                })
                .unwrap_or(0.0);

            Ok((tx, ty))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(ValueErrorKind::from)
        .and_then(|(tx, ty)| {
            let tx = f64::from(finite_f32(tx)?);
            let ty = f64::from(finite_f32(ty)?);

            Ok(Transform::row_major(1.0, 0.0, 0.0, 1.0, tx, ty))
        })
}

fn parse_scale_args(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let x = p.expect_number()?;

            let y = p
                .try_parse(|p| -> Result<f32, CssParseError<'_, ()>> {
                    p.optional_comma();
                    Ok(p.expect_number()?)
                })
                .unwrap_or(x);

            Ok((x, y))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(ValueErrorKind::from)
        .and_then(|(x, y)| {
            let x = f64::from(finite_f32(x)?);
            let y = f64::from(finite_f32(y)?);

            Ok(Transform::row_major(x, 0.0, 0.0, y, 0.0, 0.0))
        })
}

fn parse_rotate_args(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let angle = p.expect_number()?;

            let (tx, ty) = p
                .try_parse(|p| -> Result<_, CssParseError<'_, ()>> {
                    p.optional_comma();
                    let tx = p.expect_number()?;

                    p.optional_comma();
                    let ty = p.expect_number()?;

                    Ok((tx, ty))
                })
                .unwrap_or((0.0, 0.0));

            Ok((angle, tx, ty))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(ValueErrorKind::from)
        .and_then(|(angle, tx, ty)| {
            let angle = f64::from(finite_f32(angle)?);
            let tx = f64::from(finite_f32(tx)?);
            let ty = f64::from(finite_f32(ty)?);

            let angle = angle * PI / 180.0;
            let (s, c) = angle.sin_cos();

            let mut m = Transform::row_major(1.0, 0.0, 0.0, 1.0, tx, ty);

            // FIXME: use euclid pre_rotate / pre_translate?
            m = m.pre_transform(&Transform::row_major(c, s, -s, c, 0.0, 0.0));
            m = m.pre_transform(&Transform::row_major(1.0, 0.0, 0.0, 1.0, -tx, -ty));
            Ok(m)
        })
}

fn parse_skewx_args(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let a = p.expect_number()?;
            Ok(a)
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(ValueErrorKind::from)
        .and_then(|a| {
            let a = f64::from(finite_f32(a)?);

            let a = a * PI / 180.0;
            Ok(Transform::row_major(1.0, 0.0, a.tan(), 1.0, 0.0, 0.0))
        })
}

fn parse_skewy_args(parser: &mut Parser<'_, '_>) -> Result<Transform, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let a = p.expect_number()?;
            Ok(a)
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(ValueErrorKind::from)
        .and_then(|a| {
            let a = f64::from(finite_f32(a)?);

            let a = a * PI / 180.0;
            Ok(Transform::row_major(1.0, a.tan(), 0.0, 1.0, 0.0, 0.0))
        })
}

#[cfg(test)]
fn make_rotation_matrix(angle_degrees: f64, tx: f64, ty: f64) -> Transform {
    let angle = angle_degrees * PI / 180.0;

    let mut m = Transform::row_major(1.0, 0.0, 0.0, 1.0, tx, ty);

    let mut r = Transform::identity();
    r.rotate(angle);

    m = m.pre_transform(&r);
    m.pre_transform(&Transform::row_major(1.0, 0.0, 0.0, 1.0, -tx, -ty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::ApproxEq;
    use std::f64;

    #[test]
    fn transform_rect() {
        let r = cairo::Rectangle {
            x: 0.42,
            y: 0.42,
            width: 3.14,
            height: 3.14,
        };

        let t = cairo::Matrix::identity();
        let tr = t.transform_rect(&r);
        assert_eq!(tr, r);

        let t = cairo::Matrix::new(2.0, 0.0, 0.0, 2.0, 1.5, 1.5);
        let tr = r.transform(&m);
        assert_approx_eq_cairo!(2.34_f64, tr.x);
        assert_approx_eq_cairo!(2.34_f64, tr.y);
        assert_approx_eq_cairo!(6.28_f64, tr.width);
        assert_approx_eq_cairo!(6.28_f64, tr.height);
    }

    fn parse_transform(s: &str) -> Result<Transform, ValueErrorKind> {
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
        match parse_transform(s) {
            Err(ValueErrorKind::Parse(_)) => {}
            _ => {
                panic!();
            }
        }
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
        match parse_transform("matrix (0 0 0 0 0 0)") {
            Err(ValueErrorKind::Value(_)) => {}
            _ => {
                panic!();
            }
        }

        match parse_transform("scale (0), translate (10, 10)") {
            Err(ValueErrorKind::Value(_)) => {}
            _ => {
                panic!();
            }
        }

        match parse_transform("scale (0), skewX (90)") {
            Err(ValueErrorKind::Value(_)) => {}
            _ => {
                panic!();
            }
        }
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
