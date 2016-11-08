use std::str;
use std::str::Chars;
use std::iter::Enumerate;
use path_builder::*;

extern crate cairo;

pub struct PathParser<'external> {
    path_str: &'external str,
    chars_enumerator: Enumerate<Chars<'external>>,
    lookahead: Option <char>, /* None if we are in EOF */
    current_pos: usize,

    builder: &'external mut RsvgPathBuilder,

    error_message: &'static str,
    has_error: bool,

    /* Current point; adjusted at every command */
    current_x: f64,
    current_y: f64,

    /* Last control point from previous curve command, used to reflect
     * the new control point for smooth curve commands.
     */
    reflection_x: f64,
    reflection_y: f64
}

/* This is a recursive descent parser for path data in SVG files,
 * as specified in https://www.w3.org/TR/SVG/paths.html#PathDataBNF
 *
 * Some peculiarities:
 *
 * - SVG allows optional commas inside coordiante pairs, and between
 * coordinate pairs.  So, for example, these are equivalent:
 *
 *     M 10 20 30 40
 *     M 10, 20 30, 40
 *     M 10, 20, 30, 40
 *
 * - Whitespace is optional.  These are equivalent:
 *
 *     M10,20 30,40
 *     M10,20,30,40
 *
 *   These are also equivalent:
 *
 *     M-10,20-30-40
 *     M -10 20 -30 -40
 *
 *     M.1-2,3E2-4
 *     M 0.1 -2 300 -4
 */
impl<'external> PathParser<'external> {
    pub fn new (builder: &'external mut RsvgPathBuilder, path_str: &'external str) -> PathParser<'external> {
        PathParser {
            path_str: path_str,
            chars_enumerator: path_str.chars ().enumerate (),
            lookahead: None,
            current_pos: 0,

            builder: builder,

            error_message: "",
            has_error: false,

            current_x: 0.0,
            current_y: 0.0,

            reflection_x: 0.0,
            reflection_y: 0.0
        }
    }

    pub fn parse (&mut self) -> bool {
        self.getchar ();

        return self.optional_whitespace () &&
            self.moveto_drawto_command_groups () &&
            self.optional_whitespace ();
    }

    fn getchar (&mut self) {
        if let Some ((pos, c)) = self.chars_enumerator.next () {
            self.lookahead = Some (c);
            self.current_pos = pos;
        } else {
            self.lookahead = None;
            self.current_pos += 1; /* this is EOF; point just past the end the string */
        }
    }

    fn error (&mut self, message: &'static str) -> bool {
        self.error_message = message;
        self.has_error = true;
        false
    }

    fn match_char (&mut self, c: char) -> bool {
        if let Some (x) = self.lookahead {
            if c == x {
                self.getchar ();
                return true;
            }
        }

        false
    }

    fn whitespace (&mut self) -> bool {
        if let Some (c) = self.lookahead {
            if c.is_whitespace () {
                assert! (self.match_char (c));

                while let Some (c) = self.lookahead {
                    if c.is_whitespace () {
                        assert! (self.match_char (c));
                        continue;
                    } else {
                        break;
                    }
                }

                return true;
            } else {
                return false;
            }
        }

        false
    }

    fn optional_whitespace (&mut self) -> bool {
        self.whitespace ();
        true
    }

    fn optional_comma_whitespace (&mut self) -> bool {
        assert! (self.optional_whitespace ());
        self.match_char (',');
        assert! (self.optional_whitespace ());
        true
    }

    fn lookahead_is (&self, c: char) -> bool {
        if let Some (x) = self.lookahead {
            if x == c {
                return true;
            }
        }

        false
    }

    fn lookahead_is_digit (&self, d: &mut char) -> bool {
        if let Some (c) = self.lookahead {
            if c.is_digit (10) {
                *d = c;
                return true;
            }
        }

        false
    }

    fn number (&mut self) -> Option <f64> {
        let mut has_sign: bool;
        let mut value: f64;
        let mut sign: f64;
        let mut exponent_sign: f64;
        let mut exponent: f64;

        has_sign = false;
        sign = 1.0;
        value = 0.0;
        exponent_sign = 1.0;
        exponent = 0.0;

        if self.match_char ('+') {
            sign = 1.0;
            has_sign = true;
        } else if self.match_char ('-') {
            sign = -1.0;
            has_sign = true;
        }

        let mut c: char = ' ';

        if self.lookahead_is_digit (&mut c) || self.lookahead_is ('.') {
            /* Integer part */

            while self.lookahead_is_digit (&mut c) {
                value = value * 10.0 + char_to_digit (c) as f64;

                assert! (self.match_char (c));
            }

            /* Fractional part */

            if self.match_char ('.') {
                let mut fraction: f64 = 1.0;

                let mut c: char = ' ';

                while self.lookahead_is_digit (&mut c) {
                    fraction = fraction / 10.0;
                    value += fraction * char_to_digit (c) as f64;

                    assert! (self.match_char (c));
                }
            }

            if self.match_char ('E') || self.match_char ('e') {
                /* exponent sign */

                if self.match_char ('+') {
                    exponent_sign = 1.0;
                } else if self.match_char ('-') {
                    exponent_sign = -1.0;
                }

                /* exponent */

                let mut c: char = ' ';

                if self.lookahead_is_digit (&mut c) {
                    while self.lookahead_is_digit (&mut c) {
                        exponent = exponent * 10.0 + char_to_digit (c) as f64;

                        assert! (self.match_char (c));
                    }
                } else {
                    self.error ("Expected digits for exponent");
                    return None;
                }
            }

            Some (value * sign * 10.0f64.powf (exponent * exponent_sign))
        } else {
            if has_sign {
                self.error ("Expected number after sign");
            }

            None
        }
    }

    fn coordinate_pair (&mut self) -> Option<(f64, f64)> {
        if let Some (num1) = self.number () {
            assert! (self.optional_comma_whitespace ());

            if let Some (num2) = self.number () {
                return Some ((num1, num2));
            } else {
                self.error ("Expected second coordinate of coordinate pair");
                return None
            }
        }

        None
    }

    fn emit_move_to (&mut self, x: f64, y: f64) {
        self.current_x = x;
        self.current_y = y;
        self.reflection_x = self.current_x;
        self.reflection_y = self.current_y;

        self.builder.move_to (self.current_x, self.current_y);
        println! ("emitting moveto {} {}", self.current_x, self.current_y);
    }

    fn emit_line_to (&mut self, x: f64, y: f64) {
        self.current_x = x;
        self.current_y = y;
        self.reflection_x = self.current_x;
        self.reflection_y = self.current_y;

        self.builder.line_to (self.current_x, self.current_y);
        println! ("emitting lineto {} {}", self.current_x, self.current_y);
    }

    fn emit_curve_to (&mut self, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) {
        self.current_x = x4;
        self.current_y = y4;
        self.reflection_x = x3;
        self.reflection_y = y3;

        self.builder.curve_to (x2, y2, x3, y3, x4, y4);

        println! ("emitting curveto ({} {}) ({} {}) ({} {})", x2, y2, x3, y3, x4, y4);
    }

    fn lineto_argument_sequence (&mut self, absolute: bool) -> bool {
        if let Some ((mut x, mut y)) = self.coordinate_pair () {
            if !absolute {
                x += self.current_x;
                y += self.current_y;
            }

            self.emit_line_to (x, y);

            self.whitespace ();

            if self.lookahead_is (',') {
                assert! (self.match_char (','));
                assert! (self.optional_whitespace ());

                if !self.lineto_argument_sequence (absolute) {
                    self.error ("Expected coordinate pair after comma");
                    return false;
                }
            }

            self.lineto_argument_sequence (absolute);
            true
        } else {
            false
        }
    }

    fn moveto_argument_sequence (&mut self, absolute: bool, is_initial_moveto: bool) -> bool {
        if let Some ((mut x, mut y)) = self.coordinate_pair () {
            if is_initial_moveto {
                self.emit_move_to (x, y);
            } else {
                if !absolute {
                    x += self.current_x;
                    y += self.current_y;
                }

                self.emit_move_to (x, y);
            }

            self.whitespace ();

            if self.lookahead_is (',') {
                assert! (self.match_char (','));
                assert! (self.optional_whitespace ());

                if !self.lineto_argument_sequence (absolute) {
                    self.error ("Expected coordinate pair after comma");
                    return false;
                }
            }

            self.lineto_argument_sequence (absolute);
            true
        } else {
            self.error ("Expected coordinate pair after moveto")
        }
    }

    fn moveto (&mut self, is_initial_moveto: bool) -> bool {
        if self.lookahead_is ('M') || self.lookahead_is ('m') {
            let absolute: bool;

            if self.match_char ('M') {
                absolute = true;
            } else {
                assert! (self.match_char ('m'));
                absolute = false;
            }

            return self.optional_whitespace () &&
                self.moveto_argument_sequence (absolute, is_initial_moveto);
        }

        false
    }

    fn moveto_drawto_command_group (&mut self, is_initial_moveto: bool) -> bool {
        if self.moveto (is_initial_moveto) {
            return self.optional_whitespace () &&
                self.optional_drawto_commands ();
        } else {
            false
        }
    }

    fn moveto_drawto_command_groups (&mut self) -> bool {
        if self.moveto_drawto_command_group (true) {
            loop {
                self.optional_whitespace ();
                if !self.moveto_drawto_command_group (false) {
                    break;
                }
            }

            true
        } else {
            self.error ("Expected moveto command")
        }
    }

    fn optional_drawto_commands (&mut self) -> bool {
        if self.drawto_command () {
            loop {
                self.optional_whitespace ();
                if !self.drawto_command () {
                    break;
                }
            }
        }

        true
    }

    fn drawto_command (&mut self) -> bool {
        return self.close_path () ||
            self.line_to () ||
            self.horizontal_line_to () ||
            self.vertical_line_to () ||
            self.curve_to () ||
            self.smooth_curve_to () ||
            self.quadratic_bezier_curve_to () ||
            self.smooth_quadratic_bezier_curve_to () ||
            self.elliptical_arc ();
    }

    fn close_path (&mut self) -> bool {
        false
    }

    fn line_to (&mut self) -> bool {
        if self.lookahead_is ('L') || self.lookahead_is ('l') {
            let absolute: bool;

            if self.match_char ('L') {
                absolute = true;
            } else {
                assert! (self.match_char ('l'));
                absolute = false;
            }

            self.optional_whitespace ();

            if self.lineto_argument_sequence (absolute) {
                return true;
            } else {
                return self.error ("Expected coordinate pair after lineto");
            }
        }

        false
    }

    fn horizontal_lineto_argument_sequence (&mut self, absolute: bool) -> bool {
        if let Some (mut x) = self.number () {
            if !absolute {
                x += self.current_x;
            }

            let y = self.current_y;

            self.emit_line_to (x, y);

            self.whitespace ();

            if self.lookahead_is (',') {
                assert! (self.match_char (','));
                assert! (self.optional_whitespace ());

                if !self.horizontal_lineto_argument_sequence (absolute) {
                    self.error ("Expected offset after comma");
                    return false;
                }
            }

            self.horizontal_lineto_argument_sequence (absolute);
            true
        } else {
            false
        }
    }

    fn horizontal_line_to (&mut self) -> bool {
        if self.lookahead_is ('H') || self.lookahead_is ('h') {
            let absolute: bool;

            if self.match_char ('H') {
                absolute = true;
            } else {
                assert! (self.match_char ('h'));
                absolute = false;
            }

            self.optional_whitespace ();

            if self.horizontal_lineto_argument_sequence (absolute) {
                return true;
            } else {
                return self.error ("Expected offset after horizontal lineto");
            }
        }

        false
    }

    fn vertical_lineto_argument_sequence (&mut self, absolute: bool) -> bool {
        if let Some (mut y) = self.number () {
            let x = self.current_x;

            if !absolute {
                y += self.current_y;
            }

            self.emit_line_to (x, y);

            self.whitespace ();

            if self.lookahead_is (',') {
                assert! (self.match_char (','));
                assert! (self.optional_whitespace ());

                if !self.vertical_lineto_argument_sequence (absolute) {
                    self.error ("Expected offset after comma");
                    return false;
                }
            }

            self.vertical_lineto_argument_sequence (absolute);
            true
        } else {
            false
        }
    }

    fn vertical_line_to (&mut self) -> bool {
        if self.lookahead_is ('V') || self.lookahead_is ('v') {
            let absolute: bool;

            if self.match_char ('V') {
                absolute = true;
            } else {
                assert! (self.match_char ('v'));
                absolute = false;
            }

            self.optional_whitespace ();

            if self.vertical_lineto_argument_sequence (absolute) {
                return true;
            } else {
                return self.error ("Expected offset after vertical lineto");
            }
        }

        false
    }

    fn curveto_argument_sequence (&mut self, absolute: bool) -> bool {
        if let Some ((mut x2, mut y2)) = self.coordinate_pair () {
            assert! (self.optional_comma_whitespace ());

            if let Some ((mut x3, mut y3)) = self.coordinate_pair () {
                assert! (self.optional_comma_whitespace ());

                if let Some ((mut x4, mut y4)) = self.coordinate_pair () {
                    if !absolute {
                        x2 += self.current_x;
                        y2 += self.current_y;
                        x3 += self.current_x;
                        y3 += self.current_y;
                        x4 += self.current_x;
                        y4 += self.current_y;
                    }
                    self.emit_curve_to (x2, y2, x3, y3, x4, y4);

                    self.whitespace ();

                    if self.lookahead_is (',') {
                        assert! (self.match_char (','));
                        assert! (self.optional_whitespace ());

                        if !self.curveto_argument_sequence (absolute) {
                            self.error ("Expected coordinate pair after comma");
                            return false;
                        }
                    }

                    self.curveto_argument_sequence (absolute);
                    return true;
                } else {
                    return self.error ("Expected third coordinate pair for curveto");
                }
            } else {
                return self.error ("Expected second coordinate pair for curveto");
            }
        } else {
            false
        }
    }

    fn smooth_curveto_argument_sequence (&mut self, absolute: bool) -> bool {
        if let Some ((mut x3, mut y3)) = self.coordinate_pair () {
            assert! (self.optional_comma_whitespace ());

            if let Some ((mut x4, mut y4)) = self.coordinate_pair () {
                if !absolute {
                    x3 += self.current_x;
                    y3 += self.current_y;
                    x4 += self.current_x;
                    y4 += self.current_y;
                }

                let (x2, y2) = (self.current_x + self.current_x - self.reflection_x,
                                self.current_y + self.current_y - self.reflection_y);

                self.emit_curve_to (x2, y2, x3, y3, x4, y4);

                self.whitespace ();

                if self.lookahead_is (',') {
                    assert! (self.match_char (','));
                    assert! (self.optional_whitespace ());

                    if !self.smooth_curveto_argument_sequence (absolute) {
                        self.error ("Expected coordinate pair after comma");
                        return false;
                    }
                }

                self.smooth_curveto_argument_sequence (absolute);
                return true;
            } else {
                return self.error ("Expected second coordinate pair for smooth curveto");
            }
        } else {
            false
        }
    }

    fn curve_to (&mut self) -> bool {
        if self.lookahead_is ('C') || self.lookahead_is ('c') {
            let absolute: bool;

            if self.match_char ('C') {
                absolute = true;
            } else {
                assert! (self.match_char ('c'));
                absolute = false;
            }

            self.optional_whitespace ();

            if self.curveto_argument_sequence (absolute) {
                return true;
            } else {
                return self.error ("Expected coordinate pair after curveto");
            }
        }

        false
    }

    fn smooth_curve_to (&mut self) -> bool {
        if self.lookahead_is ('S') || self.lookahead_is ('s') {
            let absolute: bool;

            if self.match_char ('S') {
                absolute = true;
            } else {
                assert! (self.match_char ('s'));
                absolute = false;
            }

            self.optional_whitespace ();

            if self.smooth_curveto_argument_sequence (absolute) {
                return true;
            } else {
                return self.error ("Expected coordinate pair after smooth curveto");
            }
        }

        false
    }

    fn quadratic_bezier_curve_to (&mut self) -> bool {
        false
    }

    fn smooth_quadratic_bezier_curve_to (&mut self) -> bool {
        false
    }

    fn elliptical_arc (&mut self) -> bool {
        false
    }
}

fn char_to_digit (c: char) -> i32 {
    c as i32 - '0' as i32
}


#[cfg(test)]
mod tests {
    use super::*;
    use path_builder::*;

    extern crate cairo;

    fn path_segment_vectors_are_equal (a: &Vec<cairo::PathSegment>,
                                       b: &Vec<cairo::PathSegment>) -> bool {

        if a.len() != b.len () {
            return false;
        }

        if a.len () == 0 && b.len () == 0 {
            return true;
        }

        let mut iter = a.iter().zip (b);

        loop {
            if let Some ((seg1, seg2)) = iter.next () {
                match *seg1 {
                    cairo::PathSegment::MoveTo ((x, y)) => {
                        if let cairo::PathSegment::MoveTo ((ox, oy)) = *seg2 {
                            println! ("{} {} {} {}", x, y, ox, oy);
                            if (x, y) != (ox, oy) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    },

                    cairo::PathSegment::LineTo ((x, y)) => {
                        if let cairo::PathSegment::LineTo ((ox, oy)) = *seg2 {
                            println! ("{} {} {} {}", x, y, ox, oy);
                            if (x, y) != (ox, oy) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    },

                    cairo::PathSegment::CurveTo ((x2, y2), (x3, y3), (x4, y4)) => {
                        if let cairo::PathSegment::CurveTo ((ox2, oy2), (ox3, oy3), (ox4, oy4)) = *seg2 {
                            if (ox2, oy2, ox3, oy3, ox4, oy4) != (x2, y2, x3, y3, x4, y4) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    },

                    cairo::PathSegment::ClosePath => {
                        if let cairo::PathSegment::ClosePath = *seg2 {
                            /* okay */
                        } else {
                            return false;
                        }
                    }
                }
            } else {
                break;
            }
        }

        true
    }

    fn print_error (parser: &PathParser) {
        let prefix = "Error in \"";

        println! ("");
        println! ("{}{}\"", prefix, &parser.path_str);

        for _ in 0 .. (prefix.len() + parser.current_pos) {
            print! (" ");
        }

        println! ("^ pos {}", parser.current_pos);
        println! ("{}", &parser.error_message);
    }

    fn parse_path (path_str: &str) -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        {
            let mut parser = PathParser::new (&mut builder, path_str);
            if !parser.parse () {
                print_error (&parser);
            }
        }

        builder
    }

    fn test_parser (path_str: &str,
                    expected_segments: &Vec<cairo::PathSegment>) {
        let builder = parse_path (path_str);
        let segments = builder.get_path_segments ();

        assert! (path_segment_vectors_are_equal (expected_segments, segments));
    }

    fn moveto (x: f64, y: f64) -> cairo::PathSegment {
        cairo::PathSegment::MoveTo ((x, y))
    }

    fn lineto (x: f64, y: f64) -> cairo::PathSegment {
        cairo::PathSegment::LineTo ((x, y))
    }

    fn curveto (x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> cairo::PathSegment {
        cairo::PathSegment::CurveTo ((x2, y2), (x3, y3), (x4, y4))
    }

    fn closepath () -> cairo::PathSegment {
        cairo::PathSegment::ClosePath
    }

    #[test]
    fn path_parser_handles_empty_data () {
        test_parser ("",
                     &Vec::<cairo::PathSegment>::new ());
    }

    #[test]
    fn path_parser_handles_numbers () {
        test_parser ("M 10 20",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);

        test_parser ("M -10 -20",
                     &vec![
                         moveto (-10.0, -20.0)
                     ]);

        test_parser ("M .10 0.20",
                     &vec![
                         moveto (0.10, 0.20)
                     ]);

        test_parser ("M -.10 -0.20",
                     &vec![
                         moveto (-0.10, -0.20)
                     ]);

        test_parser ("M-.10-0.20",
                     &vec![
                         moveto (-0.10, -0.20)
                     ]);

        test_parser ("M.10.20",
                     &vec![
                         moveto (0.10, 0.20)
                     ]);

        test_parser ("M .10E1 .20e-4",
                     &vec![
                         moveto (1.0, 0.000020)
                     ]);

        test_parser ("M-.10E1-.20",
                     &vec![
                         moveto (-1.0, -0.20)
                     ]);

        test_parser ("M10.10E2 -0.20e3",
                     &vec![
                         moveto (1010.0, -200.0)
                     ]);

        test_parser ("M-10.10E2-0.20e-3",
                     &vec![
                         moveto (-1010.0, -0.00020)
                     ]);
    }

    #[test]
    fn path_parser_handles_numbers_with_comma () {
        test_parser ("M 10, 20",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);

        test_parser ("M -10,-20",
                     &vec![
                         moveto (-10.0, -20.0)
                     ]);

        test_parser ("M.10    ,    0.20",
                     &vec![
                         moveto (0.10, 0.20)
                     ]);

        test_parser ("M -.10, -0.20   ",
                     &vec![
                         moveto (-0.10, -0.20)
                     ]);

        test_parser ("M-.10-0.20",
                     &vec![
                         moveto (-0.10, -0.20)
                     ]);

        test_parser ("M.10.20",
                     &vec![
                         moveto (0.10, 0.20)
                     ]);

        test_parser ("M .10E1,.20e-4",
                     &vec![
                         moveto (1.0, 0.000020)
                     ]);

        test_parser ("M-.10E-2,-.20",
                     &vec![
                         moveto (-0.0010, -0.20)
                     ]);

        test_parser ("M10.10E2,-0.20e3",
                     &vec![
                         moveto (1010.0, -200.0)
                     ]);

        test_parser ("M-10.10E2,-0.20e-3",
                     &vec![
                         moveto (-1010.0, -0.00020)
                     ]);
    }

    #[test]
    fn path_parser_handles_single_moveto () {
        test_parser ("M 10 20",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);

        test_parser ("M10,20",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);

        test_parser ("M10 20",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);

        test_parser ("    M10,20     ",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto () {
        test_parser ("m10 20",
                     &vec![
                         moveto (10.0, 20.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_absolute_moveto_with_implicit_lineto () {
        test_parser ("M10 20 30 40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 40.0)
                     ]);

        test_parser ("M10,20,30,40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 40.0)
                     ]);

        test_parser ("M.1-2,3E2-4",
                     &vec![
                         moveto (0.1, -2.0),
                         lineto (300.0, -4.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto_with_implicit_lineto () {
        test_parser ("m10 20 30 40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (40.0, 60.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_absolute_moveto_with_implicit_linetos () {
        test_parser ("M10,20 30,40,50 60",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 40.0),
                         lineto (50.0, 60.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto_with_implicit_linetos () {
        test_parser ("m10 20 30 40 50 60",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (40.0, 60.0),
                         lineto (90.0, 120.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_absolute_moveto_moveto () {
        test_parser ("M10 20 M 30 40",
                     &vec![
                         moveto (10.0, 20.0),
                         moveto (30.0, 40.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto_moveto () {
        test_parser ("m10 20 m 30 40",
                     &vec![
                         moveto (10.0, 20.0),
                         moveto (40.0, 60.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto_lineto_moveto () {
        test_parser ("m10 20 30 40 m 50 60",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (40.0, 60.0),
                         moveto (90.0, 120.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_absolute_moveto_lineto () {
        test_parser ("M10 20 L30,40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 40.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto_lineto () {
        test_parser ("m10 20 l30,40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (40.0, 60.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_relative_moveto_lineto_lineto_abs_lineto () {
        test_parser ("m10 20 30 40,l30,40,50 60L200,300",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (40.0, 60.0),
                         lineto (70.0, 100.0),
                         lineto (120.0, 160.0),
                         lineto (200.0, 300.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_horizontal_lineto () {
        test_parser ("M10 20 H30",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 20.0)
                     ]);

        test_parser ("M10 20 H30 40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 20.0),
                         lineto (40.0, 20.0)
                     ]);

        test_parser ("M10 20 H30,40-50",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (30.0, 20.0),
                         lineto (40.0, 20.0),
                         lineto (-50.0, 20.0),
                     ]);

        test_parser ("m10 20 h30,40-50",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (40.0, 20.0),
                         lineto (80.0, 20.0),
                         lineto (30.0, 20.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_vertical_lineto () {
        test_parser ("M10 20 V30",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (10.0, 30.0)
                     ]);

        test_parser ("M10 20 V30 40",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (10.0, 30.0),
                         lineto (10.0, 40.0)
                     ]);

        test_parser ("M10 20 V30,40-50",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (10.0, 30.0),
                         lineto (10.0, 40.0),
                         lineto (10.0, -50.0),
                     ]);

        test_parser ("m10 20 v30,40-50",
                     &vec![
                         moveto (10.0, 20.0),
                         lineto (10.0, 50.0),
                         lineto (10.0, 90.0),
                         lineto (10.0, 40.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_curveto () {
        test_parser ("M10 20 C 30,40 50 60-70,80",
                     &vec![
                         moveto  (10.0, 20.0),
                         curveto (30.0, 40.0, 50.0, 60.0, -70.0, 80.0)
                     ]);

        test_parser ("M10 20 C 30,40 50 60-70,80,90 100,110 120,130,140",
                     &vec![
                         moveto (10.0, 20.0),
                         curveto (30.0, 40.0, 50.0, 60.0, -70.0, 80.0),
                         curveto (90.0, 100.0, 110.0, 120.0, 130.0, 140.0)
                     ]);

        test_parser ("m10 20 c 30,40 50 60-70,80,90 100,110 120,130,140",
                     &vec![
                         moveto (10.0, 20.0),
                         curveto (40.0, 60.0, 60.0, 80.0, -60.0, 100.0),
                         curveto (30.0, 200.0, 50.0, 220.0, 70.0, 240.0)
                     ]);

        test_parser ("m10 20 c 30,40 50 60-70,80,90 100,110 120,130,140",
                     &vec![
                         moveto (10.0, 20.0),
                         curveto (40.0, 60.0, 60.0, 80.0, -60.0, 100.0),
                         curveto (30.0, 200.0, 50.0, 220.0, 70.0, 240.0)
                     ]);
    }

    #[test]
    fn path_parser_handles_smooth_curveto () {
        test_parser ("M10 20 S 30,40-50,60",
                     &vec![
                         moveto  (10.0, 20.0),
                         curveto (10.0, 20.0, 30.0, 40.0, -50.0, 60.0)
                     ]);

        test_parser ("M10 20 S 30,40 50 60-70,80,90 100",
                     &vec![
                         moveto (10.0, 20.0),
                         curveto (10.0, 20.0, 30.0, 40.0, 50.0, 60.0),
                         curveto (70.0, 80.0, -70.0, 80.0, 90.0, 100.0)
                     ]);

        test_parser ("m10 20 s 30,40 50 60-70,80,90 100,110 120",
                     &vec![
                         moveto (10.0, 20.0),
                         curveto (10.0, 20.0, 40.0, 60.0, 60.0, 80.0),
                         curveto (80.0, 100.0, -10.0, 160.0, 150.0, 180.0)
                     ]);
    }
}
