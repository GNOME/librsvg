use ::cairo;
use ::glib::translate::*;
use ::glib_sys;
use ::libc;

#[cfg(test)]
use std::f64::consts::*;

use cairo::MatrixTrait;
use cssparser::{Parser, ParserInput, Token, BasicParseError};

use error::*;
use parsers::ParseError;
use parsers::Parse;

impl Parse for cairo::Matrix {
    type Data = ();
    type Err = AttributeError;

    fn parse (s: &str, _: ()) -> Result<cairo::Matrix, AttributeError> {
        parse_transform (s)
    }
}

pub fn parse_transform(s: &str) -> Result<cairo::Matrix, AttributeError> {
    let mut input = ParserInput::new(s);
    let mut parser = Parser::new(&mut input);

    let xform = parser.expect_ident_cloned()?;
    let _ = parser.expect_parenthesis_block()?;

    match xform.as_ref() {
        "matrix"    => parse_matrix_args(&mut parser),
        "translate" => parse_translate_args(&mut parser),
        "scale"     => parse_scale_args(&mut parser),
        "rotate"    => parse_rotate_args(&mut parser),
        "skewX"     => parse_skewX_args(&mut parser),
        "skewY"     => parse_skewY_args(&mut parser),
        _           => Err(AttributeError::from(ParseError::new("expected matrix|translate|scale|rotate|skewX|skewY"))),
    }
}

fn parse_matrix_args(parser: &mut Parser) -> Result<cairo::Matrix, AttributeError> {
    unimplemented!();
}

fn parse_translate_args(parser: &mut Parser) -> Result<cairo::Matrix, AttributeError> {
    unimplemented!();
}

fn parse_scale_args(parser: &mut Parser) -> Result<cairo::Matrix, AttributeError> {
    unimplemented!();
}

fn parse_rotate_args(parser: &mut Parser) -> Result<cairo::Matrix, AttributeError> {
    unimplemented!();
}

fn parse_skewX_args(parser: &mut Parser) -> Result<cairo::Matrix, AttributeError> {
    unimplemented!();
}

fn parse_skewY_args(parser: &mut Parser) -> Result<cairo::Matrix, AttributeError> {
    unimplemented!();
}

/*
pub fn parse_transform (s: &str) -> Result <cairo::Matrix, AttributeError> {
    let r = parse_TransformList (s);

    match r {
        Ok (m) => {
            m.try_invert ().map (|_| m)
                .map_err (|_| AttributeError::Value ("invalid transformation matrix".to_string ()))
        },

        Err (e) => {
            Err (AttributeError::Parse (ParseError::new (format! ("{:?}", e))))
        }
    }
}
*/

#[cfg(test)]
fn make_rotation_matrix (angle_degrees: f64, tx: f64, ty: f64) -> cairo::Matrix {
    let angle = angle_degrees * PI / 180.0;

    let mut m = cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, tx, ty);

    let mut r = cairo::Matrix::identity ();
    r.rotate (angle);
    m = cairo::Matrix::multiply (&r, &m);

    m = cairo::Matrix::multiply (&cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -tx, -ty), &m);
    m
}

#[no_mangle]
pub extern fn rsvg_parse_transform (out_matrix: *mut cairo::Matrix, s: *const libc::c_char) -> glib_sys::gboolean {
    assert! (!out_matrix.is_null ());
    assert! (!s.is_null ());

    let string = unsafe { String::from_glib_none (s) };
    let matrix: &mut cairo::Matrix = unsafe { &mut *out_matrix };

    match parse_transform (&string) {
        Ok (m) => {
            *matrix = m;
            true.to_glib ()
        },

        Err (_) => {
            *matrix = cairo::Matrix::identity ();
            false.to_glib ()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[ignore]
    fn parses_valid_transform () {
        let t = cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = cairo::Matrix::new (10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix (30.0, 10.0, 10.0);

        let a = cairo::Matrix::multiply (&s, &t);
        assert_eq! (parse_transform ("translate(20, 30), scale (10) rotate (30 10 10)").unwrap (),
                    cairo::Matrix::multiply (&r, &a));
    }

    #[test]
    #[ignore]
    fn syntax_error_yields_parse_error () {
        match parse_transform ("foo") {
            Err (AttributeError::Parse (_)) => {},
            _ => { panic! (); }
        }

        match parse_transform ("matrix (1 2 3 4 5)") {
            Err (AttributeError::Parse (_)) => {},
            _ => { panic! (); }
        }
    }

    #[test]
    #[ignore]
    fn invalid_transform_yields_value_error () {
        match parse_transform ("matrix (0 0 0 0 0 0)") {
            Err (AttributeError::Value (_)) => {},
            _ => { panic! (); }
        }

        match parse_transform ("scale (0), translate (10, 10)") {
            Err (AttributeError::Value (_)) => {},
            _ => { panic! (); }
        }

        match parse_transform ("scale (0), skewX (90)") {
            Err (AttributeError::Value (_)) => {},
            _ => { panic! (); }
        }
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parses_matrix () {
        assert_eq! (parse_transform ("matrix (1 2 3 4 5 6)").unwrap (),
                    cairo::Matrix::new (1.0, 2.0, 3.0, 4.0, 5.0, 6.0));

        assert_eq! (parse_transform ("matrix (1,2,3,4 5 6)").unwrap (),
                    cairo::Matrix::new (1.0, 2.0, 3.0, 4.0, 5.0, 6.0));

        assert_eq! (parse_transform ("matrix (1,2.25,-3.25e2,4 5 6)").unwrap (),
                    cairo::Matrix::new (1.0, 2.25, -325.0, 4.0, 5.0, 6.0));
    }

    #[test]
    #[ignore]
    fn parses_translate () {
        assert_eq! (parse_transform ("translate(-1 -2)").unwrap (),
                    cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -1.0, -2.0));

        assert_eq! (parse_transform ("translate(-1, -2)").unwrap (),
                    cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -1.0, -2.0));

        assert_eq! (parse_transform ("translate(-1)").unwrap (),
                    cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -1.0, 0.0));
    }

    #[test]
    #[ignore]
    fn parses_scale () {
        assert_eq! (parse_transform ("scale(-1 -2)").unwrap (),
                    cairo::Matrix::new (-1.0, 0.0, 0.0, -2.0, 0.0, 0.0));

        assert_eq! (parse_transform ("scale(-1, -2)").unwrap (),
                    cairo::Matrix::new (-1.0, 0.0, 0.0, -2.0, 0.0, 0.0));

        assert_eq! (parse_transform ("scale(-1)").unwrap (),
                    cairo::Matrix::new (-1.0, 0.0, 0.0, -1.0, 0.0, 0.0));
    }

    #[test]
    #[ignore]
    fn parses_rotate () {
        assert_eq! (parse_transform ("rotate (30)").unwrap (), make_rotation_matrix (30.0, 0.0, 0.0));
        assert_eq! (parse_transform ("rotate (30,-1,-2)").unwrap (), make_rotation_matrix (30.0, -1.0, -2.0));
        assert_eq! (parse_transform ("rotate (30, -1, -2)").unwrap (), make_rotation_matrix (30.0, -1.0, -2.0));
    }

    fn make_skew_x_matrix (angle_degrees: f64) -> cairo::Matrix {
        let a = angle_degrees * PI / 180.0;
        cairo::Matrix::new (1.0,      0.0,
                            a.tan (), 1.0,
                            0.0, 0.0)
    }

    fn make_skew_y_matrix (angle_degrees: f64) -> cairo::Matrix {
        let mut m = make_skew_x_matrix (angle_degrees);
        m.yx = m.xy;
        m.xy = 0.0;
        m
    }

    #[test]
    #[ignore]
    fn parses_skew_x () {
        assert_eq! (parse_transform ("skewX (30)").unwrap (), make_skew_x_matrix (30.0));
    }

    #[test]
    #[ignore]
    fn parses_skew_y () {
        assert_eq! (parse_transform ("skewY (30)").unwrap (), make_skew_y_matrix (30.0));
    }

    #[test]
    #[ignore]
    fn parses_transform_list () {
        let t = cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = cairo::Matrix::new (10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix (30.0, 10.0, 10.0);

        assert_eq! (parse_transform ("scale(10)rotate(30, 10, 10)").unwrap (),
                    cairo::Matrix::multiply (&r, &s));

        assert_eq! (parse_transform ("translate(20, 30), scale (10)").unwrap (),
                    cairo::Matrix::multiply (&s, &t));

        let a = cairo::Matrix::multiply (&s, &t);
        assert_eq! (parse_transform ("translate(20, 30), scale (10) rotate (30 10 10)").unwrap (),
                    cairo::Matrix::multiply (&r, &a));
    }
}
