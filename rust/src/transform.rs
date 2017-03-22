extern crate cairo;
use self::cairo::MatrixTrait;

use std::f64::consts::*;

use parse_transform::*;

#[cfg(test)]
mod parse_transform_tests {
    use super::*;

    #[test]
    fn parses_numbers () {
        assert_eq! (parse_Num ("0"),          Ok (0.0));
        assert_eq! (parse_Num ("12345"),      Ok (12345.0));
        assert_eq! (parse_Num ("-123"),       Ok (-123.0));
        assert_eq! (parse_Num ("-123.25"),    Ok (-123.25));
        assert_eq! (parse_Num ("123.25"),     Ok (123.25));
        assert_eq! (parse_Num ("-.25"),       Ok (-0.25));
        assert_eq! (parse_Num (".25"),        Ok (0.25));
        assert_eq! (parse_Num ("-25."),       Ok (-25.0));
        assert_eq! (parse_Num ("25."),        Ok (25.0));
        assert_eq! (parse_Num ("22.5e1"),     Ok (225.0));
        assert_eq! (parse_Num ("-22.5e1"),    Ok (-225.0));
        assert_eq! (parse_Num ("-123.45e2"),  Ok (-12345.0));
        assert_eq! (parse_Num ("123.45E2"),   Ok (12345.0));
        assert_eq! (parse_Num ("-123.25e-2"), Ok (-1.2325));
        assert_eq! (parse_Num ("123.25E-2"),  Ok (1.2325));
    }

    #[test]
    fn parses_matrix () {
        assert_eq! (parse_Matrix ("matrix (1 2 3 4 5 6)").unwrap (),
                    cairo::Matrix::new (1.0, 2.0, 3.0, 4.0, 5.0, 6.0));

        assert_eq! (parse_Matrix ("matrix (1,2,3,4 5 6)").unwrap (),
                    cairo::Matrix::new (1.0, 2.0, 3.0, 4.0, 5.0, 6.0));

        assert_eq! (parse_Matrix ("matrix (1,2.25,-3.25e2,4 5 6)").unwrap (),
                    cairo::Matrix::new (1.0, 2.25, -325.0, 4.0, 5.0, 6.0));
    }

    #[test]
    fn parses_translate () {
        assert_eq! (parse_Translate ("translate(-1 -2)").unwrap (),
                    cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -1.0, -2.0));

        assert_eq! (parse_Translate ("translate(-1, -2)").unwrap (),
                    cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -1.0, -2.0));

        assert_eq! (parse_Translate ("translate(-1)").unwrap (),
                    cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -1.0, 0.0));
    }

    #[test]
    fn parses_scale () {
        assert_eq! (parse_Scale ("scale(-1 -2)").unwrap (),
                    cairo::Matrix::new (-1.0, 0.0, 0.0, -2.0, 0.0, 0.0));

        assert_eq! (parse_Scale ("scale(-1, -2)").unwrap (),
                    cairo::Matrix::new (-1.0, 0.0, 0.0, -2.0, 0.0, 0.0));

        assert_eq! (parse_Scale ("scale(-1)").unwrap (),
                    cairo::Matrix::new (-1.0, 0.0, 0.0, -1.0, 0.0, 0.0));
    }

    fn make_rotation_matrix (angle_degrees: f64, tx: f64, ty: f64) -> cairo::Matrix {
        let angle = angle_degrees * PI / 180.0;

        let mut m = cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, tx, ty);

        let mut r = cairo::Matrix::identity ();
        r.rotate (angle);
        m = cairo::Matrix::multiply (&r, &m);

        m = cairo::Matrix::multiply (&cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, -tx, -ty), &m);
        m
    }

    #[test]
    fn parses_rotate () {
        assert_eq! (parse_Rotate ("rotate (30)").unwrap (), make_rotation_matrix (30.0, 0.0, 0.0));
        assert_eq! (parse_Rotate ("rotate (30,-1,-2)").unwrap (), make_rotation_matrix (30.0, -1.0, -2.0));
        assert_eq! (parse_Rotate ("rotate (30, -1, -2)").unwrap (), make_rotation_matrix (30.0, -1.0, -2.0));
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
    fn parses_skew_x () {
        assert_eq! (parse_SkewX ("skewX (30)").unwrap (), make_skew_x_matrix (30.0));
    }

    #[test]
    fn parses_skew_y () {
        assert_eq! (parse_SkewY ("skewY (30)").unwrap (), make_skew_y_matrix (30.0));
    }

    #[test]
    fn parses_transform_list () {
        let t = cairo::Matrix::new (1.0, 0.0, 0.0, 1.0, 20.0, 30.0);
        let s = cairo::Matrix::new (10.0, 0.0, 0.0, 10.0, 0.0, 0.0);
        let r = make_rotation_matrix (30.0, 10.0, 10.0);

        assert_eq! (parse_TransformList ("scale(10)rotate(30, 10, 10)").unwrap (),
                    cairo::Matrix::multiply (&r, &s));

        assert_eq! (parse_TransformList ("translate(20, 30), scale (10)").unwrap (),
                    cairo::Matrix::multiply (&s, &t));

        let a = cairo::Matrix::multiply (&s, &t);
        assert_eq! (parse_TransformList ("translate(20, 30), scale (10) rotate (30 10 10)").unwrap (),
                    cairo::Matrix::multiply (&r, &a));
    }
}
