use cairo;

use std::f64::consts::*;

use cairo::MatrixTrait;
use cssparser::{ParseError as CssParseError, Parser, Token};

use error::*;
use parsers::{finite_f32, optional_comma, Parse, ParseError};

impl Parse for cairo::Matrix {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: ()) -> Result<cairo::Matrix, ValueErrorKind> {
        let matrix = parse_transform_list(parser)?;

        matrix
            .try_invert()
            .map(|_| matrix)
            .map_err(|_| ValueErrorKind::Value("invalid transformation matrix".to_string()))
    }
}

// This parser is for the "transform" attribute in SVG.
// Its operataion and grammar are described here:
// https://www.w3.org/TR/SVG/coords.html#TransformAttribute

fn parse_transform_list(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    let mut matrix = cairo::Matrix::identity();

    loop {
        if parser.is_exhausted() {
            break;
        }

        let m = parse_transform_command(parser)?;
        matrix = cairo::Matrix::multiply(&m, &matrix);

        optional_comma(parser);
    }

    Ok(matrix)
}

fn make_expected_function_error() -> ValueErrorKind {
    ValueErrorKind::from(ParseError::new(
        "expected matrix|translate|scale|rotate|skewX|skewY",
    ))
}

fn parse_transform_command(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
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
) -> Result<cairo::Matrix, ValueErrorKind> {
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

fn parse_matrix_args(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let xx = p.expect_number()?;
            optional_comma(p);

            let yx = p.expect_number()?;
            optional_comma(p);

            let xy = p.expect_number()?;
            optional_comma(p);

            let yy = p.expect_number()?;
            optional_comma(p);

            let x0 = p.expect_number()?;
            optional_comma(p);

            let y0 = p.expect_number()?;

            Ok((xx, yx, xy, yy, x0, y0))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(|e| ValueErrorKind::from(e))
        .and_then(|(xx, yx, xy, yy, x0, y0)| {
            let xx = f64::from(finite_f32(xx)?);
            let yx = f64::from(finite_f32(yx)?);
            let xy = f64::from(finite_f32(xy)?);
            let yy = f64::from(finite_f32(yy)?);
            let x0 = f64::from(finite_f32(x0)?);
            let y0 = f64::from(finite_f32(y0)?);

            Ok(cairo::Matrix::new(xx, yx, xy, yy, x0, y0))
        })
}

fn parse_translate_args(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let tx = p.expect_number()?;

            let ty = p
                .try(|p| -> Result<f32, CssParseError<'_, ()>> {
                    optional_comma(p);
                    Ok(p.expect_number()?)
                })
                .unwrap_or(0.0);

            Ok((tx, ty))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(|e| ValueErrorKind::from(e))
        .and_then(|(tx, ty)| {
            let tx = f64::from(finite_f32(tx)?);
            let ty = f64::from(finite_f32(ty)?);

            Ok(cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty))
        })
}

fn parse_scale_args(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let x = p.expect_number()?;

            let y = p
                .try(|p| -> Result<f32, CssParseError<'_, ()>> {
                    optional_comma(p);
                    Ok(p.expect_number()?)
                })
                .unwrap_or(x);

            Ok((x, y))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(|e| ValueErrorKind::from(e))
        .and_then(|(x, y)| {
            let x = f64::from(finite_f32(x)?);
            let y = f64::from(finite_f32(y)?);

            Ok(cairo::Matrix::new(x, 0.0, 0.0, y, 0.0, 0.0))
        })
}

fn parse_rotate_args(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let angle = p.expect_number()?;

            let (tx, ty) = p
                .try(|p| -> Result<_, CssParseError<'_, ()>> {
                    optional_comma(p);
                    let tx = p.expect_number()?;

                    optional_comma(p);
                    let ty = p.expect_number()?;

                    Ok((tx, ty))
                })
                .unwrap_or((0.0, 0.0));

            Ok((angle, tx, ty))
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(|e| ValueErrorKind::from(e))
        .and_then(|(angle, tx, ty)| {
            let angle = f64::from(finite_f32(angle)?);
            let tx = f64::from(finite_f32(tx)?);
            let ty = f64::from(finite_f32(ty)?);

            let angle = angle * PI / 180.0;
            let (s, c) = angle.sin_cos();

            let mut m = cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);

            m = cairo::Matrix::multiply(&cairo::Matrix::new(c, s, -s, c, 0.0, 0.0), &m);
            m = cairo::Matrix::multiply(&cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -tx, -ty), &m);
            Ok(m)
        })
}

fn parse_skewx_args(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let a = p.expect_number()?;
            Ok(a)
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(|e| ValueErrorKind::from(e))
        .and_then(|a| {
            let a = f64::from(finite_f32(a)?);

            let a = a * PI / 180.0;
            Ok(cairo::Matrix::new(1.0, 0.0, a.tan(), 1.0, 0.0, 0.0))
        })
}

fn parse_skewy_args(parser: &mut Parser<'_, '_>) -> Result<cairo::Matrix, ValueErrorKind> {
    parser
        .parse_nested_block(|p| {
            let a = p.expect_number()?;
            Ok(a)
        })
        .map_err(CssParseError::<()>::basic)
        .map_err(|e| ValueErrorKind::from(e))
        .and_then(|a| {
            let a = f64::from(finite_f32(a)?);

            let a = a * PI / 180.0;
            Ok(cairo::Matrix::new(1.0, a.tan(), 0.0, 1.0, 0.0, 0.0))
        })
}

#[cfg(test)]
fn make_rotation_matrix(angle_degrees: f64, tx: f64, ty: f64) -> cairo::Matrix {
    let angle = angle_degrees * PI / 180.0;

    let mut m = cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);

    let mut r = cairo::Matrix::identity();
    r.rotate(angle);
    m = cairo::Matrix::multiply(&r, &m);

    m = cairo::Matrix::multiply(&cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -tx, -ty), &m);
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::ApproxEq;
    use std::f64;

    fn parse_transform(s: &str) -> Result<cairo::Matrix, ValueErrorKind> {
        cairo::Matrix::parse_str(s, ())
    }

    fn assert_matrix_eq(a: &cairo::Matrix, b: &cairo::Matrix) {
        let epsilon = 8.0 * f64::EPSILON; // kind of arbitrary, but allow for some sloppiness

        assert!(a.xx.approx_eq(&b.xx, epsilon, 1));
        assert!(a.yx.approx_eq(&b.yx, epsilon, 1));
        assert!(a.xy.approx_eq(&b.xy, epsilon, 1));
        assert!(a.yy.approx_eq(&b.yy, epsilon, 1));
        assert!(a.x0.approx_eq(&b.x0, epsilon, 1));
        assert!(a.y0.approx_eq(&b.y0, epsilon, 1));
    }

    #[test]
    fn parses_valid_transform() {
        let t = cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = cairo::Matrix::new(10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix(30.0, 10.0, 10.0);

        let a = cairo::Matrix::multiply(&s, &t);
        assert_matrix_eq(
            &parse_transform("translate(20, 30), scale (10) rotate (30 10 10)").unwrap(),
            &cairo::Matrix::multiply(&r, &a),
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
            &cairo::Matrix::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
        );

        assert_matrix_eq(
            &parse_transform("matrix(1,2,3,4 5 6)").unwrap(),
            &cairo::Matrix::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
        );

        assert_matrix_eq(
            &parse_transform("matrix (1,2.25,-3.25e2,4 5 6)").unwrap(),
            &cairo::Matrix::new(1.0, 2.25, -325.0, 4.0, 5.0, 6.0),
        );
    }

    #[test]
    fn parses_translate() {
        assert_matrix_eq(
            &parse_transform("translate(-1 -2)").unwrap(),
            &cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -1.0, -2.0),
        );

        assert_matrix_eq(
            &parse_transform("translate(-1, -2)").unwrap(),
            &cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -1.0, -2.0),
        );

        assert_matrix_eq(
            &parse_transform("translate(-1)").unwrap(),
            &cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -1.0, 0.0),
        );
    }

    #[test]
    fn parses_scale() {
        assert_matrix_eq(
            &parse_transform("scale (-1)").unwrap(),
            &cairo::Matrix::new(-1.0, 0.0, 0.0, -1.0, 0.0, 0.0),
        );

        assert_matrix_eq(
            &parse_transform("scale(-1 -2)").unwrap(),
            &cairo::Matrix::new(-1.0, 0.0, 0.0, -2.0, 0.0, 0.0),
        );

        assert_matrix_eq(
            &parse_transform("scale(-1, -2)").unwrap(),
            &cairo::Matrix::new(-1.0, 0.0, 0.0, -2.0, 0.0, 0.0),
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

    fn make_skew_x_matrix(angle_degrees: f64) -> cairo::Matrix {
        let a = angle_degrees * PI / 180.0;
        cairo::Matrix::new(1.0, 0.0, a.tan(), 1.0, 0.0, 0.0)
    }

    fn make_skew_y_matrix(angle_degrees: f64) -> cairo::Matrix {
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
        let t = cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = cairo::Matrix::new(10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix(30.0, 10.0, 10.0);

        assert_matrix_eq(
            &parse_transform("scale(10)rotate(30, 10, 10)").unwrap(),
            &cairo::Matrix::multiply(&r, &s),
        );

        assert_matrix_eq(
            &parse_transform("translate(20, 30), scale (10)").unwrap(),
            &cairo::Matrix::multiply(&s, &t),
        );

        let a = cairo::Matrix::multiply(&s, &t);
        assert_matrix_eq(
            &parse_transform("translate(20, 30), scale (10) rotate (30 10 10)").unwrap(),
            &cairo::Matrix::multiply(&r, &a),
        );
    }

    #[test]
    fn parses_empty() {
        assert_matrix_eq(&parse_transform("").unwrap(), &cairo::Matrix::identity());
    }
}
