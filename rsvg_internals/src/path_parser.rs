use path_builder::*;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::iter::Enumerate;
use std::str;
use std::str::Chars;

struct PathParser<'b> {
    chars_enumerator: Enumerate<Chars<'b>>,
    lookahead: Option<char>,    // None if we are in EOF
    current_pos: Option<usize>, // None if the string hasn't been scanned

    builder: &'b mut PathBuilder,

    // Current point; adjusted at every command
    current_x: f64,
    current_y: f64,

    // Last control point from previous cubic curve command, used to reflect
    // the new control point for smooth cubic curve commands.
    cubic_reflection_x: f64,
    cubic_reflection_y: f64,

    // Last control point from previous quadratic curve command, used to reflect
    // the new control point for smooth quadratic curve commands.
    quadratic_reflection_x: f64,
    quadratic_reflection_y: f64,

    // Start point of current subpath (i.e. position of last moveto);
    // used for closepath.
    subpath_start_x: f64,
    subpath_start_y: f64,
}

// This is a recursive descent parser for path data in SVG files,
// as specified in https://www.w3.org/TR/SVG/paths.html#PathDataBNF
// Some peculiarities:
//
// - SVG allows optional commas inside coordiante pairs, and between
// coordinate pairs.  So, for example, these are equivalent:
//
//     M 10 20 30 40
//     M 10, 20 30, 40
//     M 10, 20, 30, 40
//
// - Whitespace is optional.  These are equivalent:
//
//     M10,20 30,40
//     M10,20,30,40
//
//   These are also equivalent:
//
//     M-10,20-30-40
//     M -10 20 -30 -40
//
//     M.1-2,3E2-4
//     M 0.1 -2 300 -4
impl<'b> PathParser<'b> {
    fn new(builder: &'b mut PathBuilder, path_str: &'b str) -> PathParser<'b> {
        PathParser {
            chars_enumerator: path_str.chars().enumerate(),
            lookahead: None,
            current_pos: None,

            builder,

            current_x: 0.0,
            current_y: 0.0,

            cubic_reflection_x: 0.0,
            cubic_reflection_y: 0.0,

            quadratic_reflection_x: 0.0,
            quadratic_reflection_y: 0.0,

            subpath_start_x: 0.0,
            subpath_start_y: 0.0,
        }
    }

    fn parse(&mut self) -> Result<(), ParseError> {
        self.getchar();

        self.optional_whitespace()?;
        self.moveto_drawto_command_groups()
    }

    fn getchar(&mut self) {
        if let Some((pos, c)) = self.chars_enumerator.next() {
            self.lookahead = Some(c);
            self.current_pos = Some(pos);
        } else {
            // We got to EOF; make current_pos point to the position after the last char in the
            // string
            self.lookahead = None;
            if self.current_pos.is_none() {
                self.current_pos = Some(0);
            } else {
                self.current_pos = Some(self.current_pos.unwrap() + 1);
            }
        }
    }

    fn error(&self, kind: ErrorKind) -> ParseError {
        ParseError {
            position: self.current_pos.unwrap(),
            kind,
        }
    }

    fn match_char(&mut self, c: char) -> bool {
        if let Some(x) = self.lookahead {
            if c == x {
                self.getchar();
                return true;
            }
        }

        false
    }

    fn whitespace(&mut self) -> Result<(), ParseError> {
        if let Some(c) = self.lookahead {
            if c.is_whitespace() {
                assert!(self.match_char(c));

                while let Some(c) = self.lookahead {
                    if c.is_whitespace() {
                        assert!(self.match_char(c));
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn optional_whitespace(&mut self) -> Result<(), ParseError> {
        let _ = self.whitespace();
        Ok(())
    }

    fn optional_comma_whitespace(&mut self) -> Result<(), ParseError> {
        self.optional_whitespace()?;
        if self.lookahead_is(',') {
            self.match_char(',');
            self.optional_whitespace()?;
        }
        Ok(())
    }

    fn lookahead_is(&self, c: char) -> bool {
        if let Some(x) = self.lookahead {
            if x == c {
                return true;
            }
        }

        false
    }

    fn lookahead_is_digit(&self, d: &mut char) -> bool {
        if let Some(c) = self.lookahead {
            if c.is_digit(10) {
                *d = c;
                return true;
            }
        }

        false
    }

    fn lookahead_is_start_of_number(&mut self) -> bool {
        let mut c = ' ';
        self.lookahead_is_digit(&mut c)
            || self.lookahead_is('.')
            || self.lookahead_is('+')
            || self.lookahead_is('-')
    }

    fn number(&mut self) -> Result<f64, ParseError> {
        let mut sign: f64;

        sign = 1.0;

        if self.match_char('+') {
            sign = 1.0;
        } else if self.match_char('-') {
            sign = -1.0;
        }

        let mut has_integer_part = false;
        let mut value: f64;
        let mut exponent_sign: f64;
        let mut exponent: Option<f64>;

        value = 0.0;
        exponent_sign = 1.0;
        exponent = None;

        let mut c: char = ' ';

        if self.lookahead_is_digit(&mut c) || self.lookahead_is('.') {
            // Integer part
            while self.lookahead_is_digit(&mut c) {
                has_integer_part = true;
                value = value * 10.0 + f64::from(char_to_digit(c));

                assert!(self.match_char(c));
            }

            // Fractional part
            if self.match_char('.') {
                let mut fraction: f64 = 1.0;

                let mut c: char = ' ';

                if !has_integer_part {
                    if !self.lookahead_is_digit(&mut c) {
                        return Err(self.error(ErrorKind::UnexpectedToken));
                    }
                }

                while self.lookahead_is_digit(&mut c) {
                    fraction /= 10.0;
                    value += fraction * f64::from(char_to_digit(c));

                    assert!(self.match_char(c));
                }
            }

            if self.match_char('E') || self.match_char('e') {
                // exponent sign
                if self.match_char('+') {
                    exponent_sign = 1.0;
                } else if self.match_char('-') {
                    exponent_sign = -1.0;
                }

                // exponent
                let mut c: char = ' ';

                if self.lookahead_is_digit(&mut c) {
                    let mut exp = 0.0;

                    while self.lookahead_is_digit(&mut c) {
                        exp = exp * 10.0 + f64::from(char_to_digit(c));

                        assert!(self.match_char(c));
                    }

                    exponent = Some(exp);
                } else if self.lookahead.is_some() {
                    return Err(self.error(ErrorKind::UnexpectedToken));
                } else {
                    return Err(self.error(ErrorKind::UnexpectedEof));
                }
            }

            if let Some(exp) = exponent {
                Ok(sign * value * 10.0f64.powf(exp * exponent_sign))
            } else {
                Ok(sign * value)
            }
        } else if self.lookahead.is_some() {
            Err(self.error(ErrorKind::UnexpectedToken))
        } else {
            Err(self.error(ErrorKind::UnexpectedEof))
        }
    }

    fn flag(&mut self) -> Result<bool, ParseError> {
        if self.match_char('0') {
            Ok(false)
        } else if self.match_char('1') {
            Ok(true)
        } else if self.lookahead.is_some() {
            Err(self.error(ErrorKind::UnexpectedToken))
        } else {
            Err(self.error(ErrorKind::UnexpectedEof))
        }
    }

    fn coordinate_pair(&mut self) -> Result<(f64, f64), ParseError> {
        let a = self.number()?;
        self.optional_comma_whitespace()?;
        let b = self.number()?;

        Ok((a, b))
    }

    fn set_current_point(&mut self, x: f64, y: f64) {
        self.current_x = x;
        self.current_y = y;

        self.cubic_reflection_x = self.current_x;
        self.cubic_reflection_y = self.current_y;

        self.quadratic_reflection_x = self.current_x;
        self.quadratic_reflection_y = self.current_y;
    }

    fn set_cubic_reflection_and_current_point(&mut self, x3: f64, y3: f64, x4: f64, y4: f64) {
        self.cubic_reflection_x = x3;
        self.cubic_reflection_y = y3;

        self.current_x = x4;
        self.current_y = y4;

        self.quadratic_reflection_x = self.current_x;
        self.quadratic_reflection_y = self.current_y;
    }

    fn set_quadratic_reflection_and_current_point(&mut self, a: f64, b: f64, c: f64, d: f64) {
        self.quadratic_reflection_x = a;
        self.quadratic_reflection_y = b;

        self.current_x = c;
        self.current_y = d;

        self.cubic_reflection_x = self.current_x;
        self.cubic_reflection_y = self.current_y;
    }

    fn emit_move_to(&mut self, x: f64, y: f64) {
        self.set_current_point(x, y);

        self.subpath_start_x = self.current_x;
        self.subpath_start_y = self.current_y;

        self.builder.move_to(self.current_x, self.current_y);
    }

    fn emit_line_to(&mut self, x: f64, y: f64) {
        self.set_current_point(x, y);

        self.builder.line_to(self.current_x, self.current_y);
    }

    fn emit_curve_to(&mut self, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) {
        self.set_cubic_reflection_and_current_point(x3, y3, x4, y4);

        self.builder.curve_to(x2, y2, x3, y3, x4, y4);
    }

    fn emit_quadratic_curve_to(&mut self, a: f64, b: f64, c: f64, d: f64) {
        // raise quadratic Bézier to cubic
        let x2 = (self.current_x + 2.0 * a) / 3.0;
        let y2 = (self.current_y + 2.0 * b) / 3.0;
        let x4 = c;
        let y4 = d;
        let x3 = (x4 + 2.0 * a) / 3.0;
        let y3 = (y4 + 2.0 * b) / 3.0;

        self.set_quadratic_reflection_and_current_point(a, b, c, d);

        self.builder.curve_to(x2, y2, x3, y3, x4, y4);
    }

    fn emit_arc(
        &mut self,
        rx: f64,
        ry: f64,
        x_axis_rotation: f64,
        large_arc: LargeArc,
        sweep: Sweep,
        x: f64,
        y: f64,
    ) {
        let (start_x, start_y) = (self.current_x, self.current_y);

        self.set_current_point(x, y);

        self.builder.arc(
            start_x,
            start_y,
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            self.current_x,
            self.current_y,
        );
    }

    fn emit_close_path(&mut self) {
        let (x, y) = (self.subpath_start_x, self.subpath_start_y);
        self.set_current_point(x, y);

        self.builder.close_path();
    }

    fn lineto_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let (mut x, mut y) = self.coordinate_pair()?;

            if !absolute {
                x += self.current_x;
                y += self.current_y;
            }

            self.emit_line_to(x, y);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn moveto_argument_sequence(
        &mut self,
        absolute: bool,
        is_initial_moveto: bool,
    ) -> Result<(), ParseError> {
        let (mut x, mut y) = self.coordinate_pair()?;

        if is_initial_moveto {
            self.emit_move_to(x, y);
        } else {
            if !absolute {
                x += self.current_x;
                y += self.current_y;
            }

            self.emit_move_to(x, y);
        }

        self.whitespace()?;

        if self.lookahead_is(',') {
            assert!(self.match_char(','));
            self.optional_whitespace()?;
            self.lineto_argument_sequence(absolute)
        } else if self.lookahead_is_start_of_number() {
            self.lineto_argument_sequence(absolute)
        } else {
            Ok(())
        }
    }

    fn moveto(&mut self, is_initial_moveto: bool) -> Result<(), ParseError> {
        if self.lookahead_is('M') || self.lookahead_is('m') {
            let absolute = if self.match_char('M') {
                true
            } else {
                assert!(self.match_char('m'));
                false
            };

            self.optional_whitespace()?;
            self.moveto_argument_sequence(absolute, is_initial_moveto)
        } else if self.lookahead.is_some() {
            Err(self.error(ErrorKind::UnexpectedToken))
        } else {
            Err(self.error(ErrorKind::UnexpectedEof))
        }
    }

    fn moveto_drawto_command_group(&mut self, is_initial_moveto: bool) -> Result<(), ParseError> {
        self.moveto(is_initial_moveto)?;
        self.optional_whitespace()?;

        self.optional_drawto_commands().map(|_| ())
    }

    fn moveto_drawto_command_groups(&mut self) -> Result<(), ParseError> {
        let mut initial = true;

        loop {
            self.moveto_drawto_command_group(initial)?;
            initial = false;

            self.optional_whitespace()?;
            if self.lookahead.is_none() {
                break;
            }
        }

        Ok(())
    }

    fn optional_drawto_commands(&mut self) -> Result<bool, ParseError> {
        while self.drawto_command()? {
            self.optional_whitespace()?;
        }

        Ok(false)
    }

    fn drawto_command(&mut self) -> Result<bool, ParseError> {
        Ok(self.close_path()?
            || self.line_to()?
            || self.horizontal_line_to()?
            || self.vertical_line_to()?
            || self.curve_to()?
            || self.smooth_curve_to()?
            || self.quadratic_bezier_curve_to()?
            || self.smooth_quadratic_bezier_curve_to()?
            || self.elliptical_arc()?)
    }

    fn close_path(&mut self) -> Result<bool, ParseError> {
        if self.match_char('Z') || self.match_char('z') {
            self.emit_close_path();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn line_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('L') || self.lookahead_is('l') {
            let absolute = if self.match_char('L') {
                true
            } else {
                assert!(self.match_char('l'));
                false
            };

            self.optional_whitespace()?;

            self.lineto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn horizontal_lineto_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let mut x = self.number()?;

            if !absolute {
                x += self.current_x;
            }

            let y = self.current_y;

            self.emit_line_to(x, y);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn horizontal_line_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('H') || self.lookahead_is('h') {
            let absolute = if self.match_char('H') {
                true
            } else {
                assert!(self.match_char('h'));
                false
            };

            self.optional_whitespace()?;

            self.horizontal_lineto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn vertical_lineto_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let mut y = self.number()?;

            if !absolute {
                y += self.current_y;
            }

            let x = self.current_x;

            self.emit_line_to(x, y);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn vertical_line_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('V') || self.lookahead_is('v') {
            let absolute = if self.match_char('V') {
                true
            } else {
                assert!(self.match_char('v'));
                false
            };

            self.optional_whitespace()?;

            self.vertical_lineto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn curveto_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let (mut x2, mut y2) = self.coordinate_pair()?;

            self.optional_comma_whitespace()?;

            let (mut x3, mut y3) = self.coordinate_pair()?;

            self.optional_comma_whitespace()?;

            let (mut x4, mut y4) = self.coordinate_pair()?;

            if !absolute {
                x2 += self.current_x;
                y2 += self.current_y;
                x3 += self.current_x;
                y3 += self.current_y;
                x4 += self.current_x;
                y4 += self.current_y;
            }

            self.emit_curve_to(x2, y2, x3, y3, x4, y4);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn smooth_curveto_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let (mut x3, mut y3) = self.coordinate_pair()?;

            self.optional_comma_whitespace()?;

            let (mut x4, mut y4) = self.coordinate_pair()?;

            if !absolute {
                x3 += self.current_x;
                y3 += self.current_y;
                x4 += self.current_x;
                y4 += self.current_y;
            }

            let (x2, y2) = (
                self.current_x + self.current_x - self.cubic_reflection_x,
                self.current_y + self.current_y - self.cubic_reflection_y,
            );

            self.emit_curve_to(x2, y2, x3, y3, x4, y4);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn curve_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('C') || self.lookahead_is('c') {
            let absolute = if self.match_char('C') {
                true
            } else {
                assert!(self.match_char('c'));
                false
            };

            self.optional_whitespace()?;

            self.curveto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn smooth_curve_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('S') || self.lookahead_is('s') {
            let absolute = if self.match_char('S') {
                true
            } else {
                assert!(self.match_char('s'));
                false
            };

            self.optional_whitespace()?;

            self.smooth_curveto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn quadratic_curveto_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let (mut a, mut b) = self.coordinate_pair()?;

            self.optional_comma_whitespace()?;

            let (mut c, mut d) = self.coordinate_pair()?;

            if !absolute {
                a += self.current_x;
                b += self.current_y;
                c += self.current_x;
                d += self.current_y;
            }

            self.emit_quadratic_curve_to(a, b, c, d);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn quadratic_bezier_curve_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('Q') || self.lookahead_is('q') {
            let absolute = if self.match_char('Q') {
                true
            } else {
                assert!(self.match_char('q'));
                false
            };

            self.optional_whitespace()?;

            self.quadratic_curveto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn smooth_quadratic_curveto_argument_sequence(
        &mut self,
        absolute: bool,
    ) -> Result<(), ParseError> {
        loop {
            let (mut c, mut d) = self.coordinate_pair()?;

            if !absolute {
                c += self.current_x;
                d += self.current_y;
            }

            let (a, b) = (
                self.current_x + self.current_x - self.quadratic_reflection_x,
                self.current_y + self.current_y - self.quadratic_reflection_y,
            );

            self.emit_quadratic_curve_to(a, b, c, d);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn smooth_quadratic_bezier_curve_to(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('T') || self.lookahead_is('t') {
            let absolute = if self.match_char('T') {
                true
            } else {
                assert!(self.match_char('t'));
                false
            };

            self.optional_whitespace()?;

            self.smooth_quadratic_curveto_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn elliptical_arc_argument_sequence(&mut self, absolute: bool) -> Result<(), ParseError> {
        loop {
            let rx = self.number()?.abs();

            self.optional_comma_whitespace()?;

            let ry = self.number()?.abs();

            self.optional_comma_whitespace()?;

            let x_axis_rotation = self.number()?;

            self.optional_comma_whitespace()?;

            let large_arc = LargeArc(self.flag()?);

            self.optional_comma_whitespace()?;

            let sweep = if self.flag()? {
                Sweep::Positive
            } else {
                Sweep::Negative
            };

            self.optional_comma_whitespace()?;

            let (mut x, mut y) = self.coordinate_pair()?;

            if !absolute {
                x += self.current_x;
                y += self.current_y;
            }

            self.emit_arc(rx, ry, x_axis_rotation, large_arc, sweep, x, y);

            self.whitespace()?;

            if self.lookahead_is(',') {
                assert!(self.match_char(','));
                self.optional_whitespace()?;
            } else if !self.lookahead_is_start_of_number() {
                break;
            }
        }

        Ok(())
    }

    fn elliptical_arc(&mut self) -> Result<bool, ParseError> {
        if self.lookahead_is('A') || self.lookahead_is('a') {
            let absolute = if self.match_char('A') {
                true
            } else {
                assert!(self.match_char('a'));
                false
            };

            self.optional_whitespace()?;

            self.elliptical_arc_argument_sequence(absolute)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn char_to_digit(c: char) -> i32 {
    c as i32 - '0' as i32
}

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    UnexpectedToken,
    UnexpectedEof,
}

#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub position: usize,
    pub kind: ErrorKind,
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::UnexpectedToken => "unexpected token",
            ErrorKind::UnexpectedEof => "unexpected end of data",
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error at position {}: {}",
            self.position,
            self.description()
        )
    }
}

pub fn parse_path_into_builder(
    path_str: &str,
    builder: &mut PathBuilder,
) -> Result<(), ParseError> {
    let mut parser = PathParser::new(builder, path_str);

    parser.parse()
}

#[cfg(test)]
#[cfg_attr(rustfmt, rustfmt_skip)]
mod tests {
    use super::*;

    fn find_error_pos(s: &str) -> Option<usize> {
        s.find('^')
    }

    fn make_parse_result(
        error_pos_str: &str,
        error_kind: Option<ErrorKind>,
    ) -> Result<(), ParseError> {
        if let Some(pos) = find_error_pos(error_pos_str) {
            Err(ParseError {
                position: pos,
                kind: error_kind.unwrap(),
            })
        } else {
            assert!(error_kind.is_none());
            Ok(())
        }
    }

    fn test_parser(
        path_str: &str,
        error_pos_str: &str,
        expected_commands: &[PathCommand],
        expected_error_kind: Option<ErrorKind>,
    ) {
        let expected_result = make_parse_result(error_pos_str, expected_error_kind);

        let mut builder = PathBuilder::new();
        let result = parse_path_into_builder(path_str, &mut builder);

        let commands = builder.get_path_commands();

        assert_eq!(expected_commands, commands);
        assert_eq!(expected_result, result);
    }

    fn moveto(x: f64, y: f64) -> PathCommand {
        PathCommand::MoveTo(x, y)
    }

    fn lineto(x: f64, y: f64) -> PathCommand {
        PathCommand::LineTo(x, y)
    }

    fn curveto(x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> PathCommand {
        PathCommand::CurveTo(CubicBezierCurve {
            pt1: (x2, y2),
            pt2: (x3, y3),
            to: (x4, y4),
        })
    }

    fn closepath() -> PathCommand {
        PathCommand::ClosePath
    }

    #[test]
    fn handles_empty_data() {
        test_parser(
            "",
            "^",
            &Vec::<PathCommand>::new(),
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn handles_numbers() {
        test_parser(
            "M 10 20",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );

        test_parser(
            "M -10 -20",
            "",
            &vec![moveto(-10.0, -20.0)],
            None,
        );

        test_parser(
            "M .10 0.20",
            "",
            &vec![moveto(0.10, 0.20)],
            None,
        );

        test_parser(
            "M -.10 -0.20",
            "",
            &vec![moveto(-0.10, -0.20)],
            None,
        );

        test_parser(
            "M-.10-0.20",
            "",
            &vec![moveto(-0.10, -0.20)],
            None,
        );

        test_parser(
            "M10.5.50",
            "",
            &vec![moveto(10.5, 0.50)],
            None,
        );

        test_parser(
            "M.10.20",
            "",
            &vec![moveto(0.10, 0.20)],
            None,
        );

        test_parser(
            "M .10E1 .20e-4",
            "",
            &vec![moveto(1.0, 0.000020)],
            None,
        );

        test_parser(
            "M-.10E1-.20",
            "",
            &vec![moveto(-1.0, -0.20)],
            None,
        );

        test_parser(
            "M10.10E2 -0.20e3",
            "",
            &vec![moveto(1010.0, -200.0)],
            None,
        );

        test_parser(
            "M-10.10E2-0.20e-3",
            "",
            &vec![moveto(-1010.0, -0.00020)],
            None,
        );
    }

    #[test]
    fn detects_bogus_numbers() {
        test_parser(
            "M+",
            "  ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M-",
            "  ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M+x",
            "  ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10e",
            "    ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10ex",
            "    ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10e-",
            "     ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10e+x",
            "     ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );
    }

    #[test]
    fn handles_numbers_with_comma() {
        test_parser(
            "M 10, 20",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );

        test_parser(
            "M -10,-20",
            "",
            &vec![moveto(-10.0, -20.0)],
            None,
        );

        test_parser(
            "M.10    ,    0.20",
            "",
            &vec![moveto(0.10, 0.20)],
            None,
        );

        test_parser(
            "M -.10, -0.20   ",
            "",
            &vec![moveto(-0.10, -0.20)],
            None,
        );

        test_parser(
            "M-.10-0.20",
            "",
            &vec![moveto(-0.10, -0.20)],
            None,
        );

        test_parser(
            "M.10.20",
            "",
            &vec![moveto(0.10, 0.20)],
            None,
        );

        test_parser(
            "M .10E1,.20e-4",
            "",
            &vec![moveto(1.0, 0.000020)],
            None,
        );

        test_parser(
            "M-.10E-2,-.20",
            "",
            &vec![moveto(-0.0010, -0.20)],
            None,
        );

        test_parser(
            "M10.10E2,-0.20e3",
            "",
            &vec![moveto(1010.0, -200.0)],
            None,
        );

        test_parser(
            "M-10.10E2,-0.20e-3",
            "",
            &vec![moveto(-1010.0, -0.00020)],
            None,
        );
    }

    #[test]
    fn handles_single_moveto() {
        test_parser(
            "M 10 20 ",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );

        test_parser(
            "M10,20  ",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );

        test_parser(
            "M10 20   ",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );

        test_parser(
            "    M10,20     ",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto() {
        test_parser(
            "m10 20",
            "",
            &vec![moveto(10.0, 20.0)],
            None,
        );
    }

    #[test]
    fn handles_absolute_moveto_with_implicit_lineto() {
        test_parser(
            "M10 20 30 40",
            "",
            &vec![moveto(10.0, 20.0), lineto(30.0, 40.0)],
            None,
        );

        test_parser(
            "M10,20,30,40",
            "",
            &vec![moveto(10.0, 20.0), lineto(30.0, 40.0)],
            None,
        );

        test_parser(
            "M.1-2,3E2-4",
            "",
            &vec![moveto(0.1, -2.0), lineto(300.0, -4.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto_with_implicit_lineto() {
        test_parser(
            "m10 20 30 40",
            "",
            &vec![moveto(10.0, 20.0), lineto(40.0, 60.0)],
            None,
        );
    }

    #[test]
    fn handles_absolute_moveto_with_implicit_linetos() {
        test_parser(
            "M10,20 30,40,50 60",
            "",
            &vec![moveto(10.0, 20.0), lineto(30.0, 40.0), lineto(50.0, 60.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto_with_implicit_linetos() {
        test_parser(
            "m10 20 30 40 50 60",
            "",
            &vec![moveto(10.0, 20.0), lineto(40.0, 60.0), lineto(90.0, 120.0)],
            None,
        );
    }

    #[test]
    fn handles_absolute_moveto_moveto() {
        test_parser(
            "M10 20 M 30 40",
            "",
            &vec![moveto(10.0, 20.0), moveto(30.0, 40.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto_moveto() {
        test_parser(
            "m10 20 m 30 40",
            "",
            &vec![moveto(10.0, 20.0), moveto(40.0, 60.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto_lineto_moveto() {
        test_parser(
            "m10 20 30 40 m 50 60",
            "",
            &vec![moveto(10.0, 20.0), lineto(40.0, 60.0), moveto(90.0, 120.0)],
            None,
        );
    }

    #[test]
    fn handles_absolute_moveto_lineto() {
        test_parser(
            "M10 20 L30,40",
            "",
            &vec![moveto(10.0, 20.0), lineto(30.0, 40.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto_lineto() {
        test_parser(
            "m10 20 l30,40",
            "",
            &vec![moveto(10.0, 20.0), lineto(40.0, 60.0)],
            None,
        );
    }

    #[test]
    fn handles_relative_moveto_lineto_lineto_abs_lineto() {
        test_parser(
            "m10 20 30 40l30,40,50 60L200,300",
            "",
            &vec![
                moveto(10.0, 20.0),
                lineto(40.0, 60.0),
                lineto(70.0, 100.0),
                lineto(120.0, 160.0),
                lineto(200.0, 300.0),
            ],
            None,
        );
    }

    #[test]
    fn handles_horizontal_lineto() {
        test_parser(
            "M10 20 H30",
            "",
            &vec![moveto(10.0, 20.0), lineto(30.0, 20.0)],
            None,
        );

        test_parser(
            "M10 20 H30 40",
            "",
            &vec![moveto(10.0, 20.0), lineto(30.0, 20.0), lineto(40.0, 20.0)],
            None,
        );

        test_parser(
            "M10 20 H30,40-50",
            "",
            &vec![
                moveto(10.0, 20.0),
                lineto(30.0, 20.0),
                lineto(40.0, 20.0),
                lineto(-50.0, 20.0),
            ],
            None,
        );

        test_parser(
            "m10 20 h30,40-50",
            "",
            &vec![
                moveto(10.0, 20.0),
                lineto(40.0, 20.0),
                lineto(80.0, 20.0),
                lineto(30.0, 20.0),
            ],
            None,
        );
    }

    #[test]
    fn handles_vertical_lineto() {
        test_parser(
            "M10 20 V30",
            "",
            &vec![moveto(10.0, 20.0), lineto(10.0, 30.0)],
            None,
        );

        test_parser(
            "M10 20 V30 40",
            "",
            &vec![moveto(10.0, 20.0), lineto(10.0, 30.0), lineto(10.0, 40.0)],
            None,
        );

        test_parser(
            "M10 20 V30,40-50",
            "",
            &vec![
                moveto(10.0, 20.0),
                lineto(10.0, 30.0),
                lineto(10.0, 40.0),
                lineto(10.0, -50.0),
            ],
            None,
        );

        test_parser(
            "m10 20 v30,40-50",
            "",
            &vec![
                moveto(10.0, 20.0),
                lineto(10.0, 50.0),
                lineto(10.0, 90.0),
                lineto(10.0, 40.0),
            ],
            None,
        );
    }

    #[test]
    fn handles_curveto() {
        test_parser(
            "M10 20 C 30,40 50 60-70,80",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(30.0, 40.0, 50.0, 60.0, -70.0, 80.0),
            ],
            None,
        );

        test_parser(
            "M10 20 C 30,40 50 60-70,80,90 100,110 120,130,140",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(30.0, 40.0, 50.0, 60.0, -70.0, 80.0),
                curveto(90.0, 100.0, 110.0, 120.0, 130.0, 140.0),
            ],
            None,
        );

        test_parser(
            "m10 20 c 30,40 50 60-70,80,90 100,110 120,130,140",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(40.0, 60.0, 60.0, 80.0, -60.0, 100.0),
                curveto(30.0, 200.0, 50.0, 220.0, 70.0, 240.0),
            ],
            None,
        );

        test_parser(
            "m10 20 c 30,40 50 60-70,80,90 100,110 120,130,140",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(40.0, 60.0, 60.0, 80.0, -60.0, 100.0),
                curveto(30.0, 200.0, 50.0, 220.0, 70.0, 240.0),
            ],
            None,
        );
    }

    #[test]
    fn handles_smooth_curveto() {
        test_parser(
            "M10 20 S 30,40-50,60",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(10.0, 20.0, 30.0, 40.0, -50.0, 60.0),
            ],
            None,
        );

        test_parser(
            "M10 20 S 30,40 50 60-70,80,90 100",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(10.0, 20.0, 30.0, 40.0, 50.0, 60.0),
                curveto(70.0, 80.0, -70.0, 80.0, 90.0, 100.0),
            ],
            None,
        );

        test_parser(
            "m10 20 s 30,40 50 60-70,80,90 100",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(10.0, 20.0, 40.0, 60.0, 60.0, 80.0),
                curveto(80.0, 100.0, -10.0, 160.0, 150.0, 180.0),
            ],
            None,
        );
    }

    #[test]
    fn handles_quadratic_curveto() {
        test_parser(
            "M10 20 Q30 40 50 60",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(
                    70.0 / 3.0,
                    100.0 / 3.0,
                    110.0 / 3.0,
                    140.0 / 3.0,
                    50.0,
                    60.0,
                ),
            ],
            None,
        );

        test_parser(
            "M10 20 Q30 40 50 60,70,80-90 100",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(
                    70.0 / 3.0,
                    100.0 / 3.0,
                    110.0 / 3.0,
                    140.0 / 3.0,
                    50.0,
                    60.0,
                ),
                curveto(
                    190.0 / 3.0,
                    220.0 / 3.0,
                    50.0 / 3.0,
                    260.0 / 3.0,
                    -90.0,
                    100.0,
                ),
            ],
            None,
        );

        test_parser(
            "m10 20 q 30,40 50 60-70,80 90 100",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(
                    90.0 / 3.0,
                    140.0 / 3.0,
                    140.0 / 3.0,
                    200.0 / 3.0,
                    60.0,
                    80.0,
                ),
                curveto(
                    40.0 / 3.0,
                    400.0 / 3.0,
                    130.0 / 3.0,
                    500.0 / 3.0,
                    150.0,
                    180.0,
                ),
            ],
            None,
        );
    }

    #[test]
    fn handles_smooth_quadratic_curveto() {
        test_parser(
            "M10 20 T30 40",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(10.0, 20.0, 50.0 / 3.0, 80.0 / 3.0, 30.0, 40.0),
            ],
            None,
        );

        test_parser(
            "M10 20 Q30 40 50 60 T70 80",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(
                    70.0 / 3.0,
                    100.0 / 3.0,
                    110.0 / 3.0,
                    140.0 / 3.0,
                    50.0,
                    60.0,
                ),
                curveto(190.0 / 3.0, 220.0 / 3.0, 70.0, 80.0, 70.0, 80.0),
            ],
            None,
        );

        test_parser(
            "m10 20 q 30,40 50 60t-70,80",
            "",
            &vec![
                moveto(10.0, 20.0),
                curveto(
                    90.0 / 3.0,
                    140.0 / 3.0,
                    140.0 / 3.0,
                    200.0 / 3.0,
                    60.0,
                    80.0,
                ),
                curveto(220.0 / 3.0, 280.0 / 3.0, 50.0, 120.0, -10.0, 160.0),
            ],
            None,
        );
    }

    #[test]
    fn handles_close_path() {
        test_parser("M10 20 Z", "", &vec![moveto(10.0, 20.0), closepath()], None);

        test_parser(
            "m10 20 30 40 m 50 60 70 80 90 100z",
            "",
            &vec![
                moveto(10.0, 20.0),
                lineto(40.0, 60.0),
                moveto(90.0, 120.0),
                lineto(160.0, 200.0),
                lineto(250.0, 300.0),
                closepath(),
            ],
            None,
        );
    }

    #[test]
    fn first_command_must_be_moveto() {
        test_parser(
            "  L10 20",
            "  ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );
    }

    #[test]
    fn moveto_args() {
        test_parser(
            "M",
            " ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M,",
            " ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10",
            "   ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10,",
            "    ^",
            &vec![],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10x",
            "   ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10,x",
            "    ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );
    }

    #[test]
    fn moveto_implicit_lineto_args() {
        test_parser(
            "M10-20,",
            "       ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20-30",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20-30 x",
            "          ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedToken),
        );
    }

    #[test]
    fn closepath_no_args() {
        test_parser(
            "M10-20z10",
            "       ^",
            &vec![moveto(10.0, -20.0), closepath()],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10-20z,",
            "       ^",
            &vec![moveto(10.0, -20.0), closepath()],
            Some(ErrorKind::UnexpectedToken),
        );
    }

    #[test]
    fn lineto_args() {
        test_parser(
            "M10-20L10",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M 10,10 L 20,20,30",
            "                  ^",
            &vec![moveto(10.0, 10.0), lineto(20.0, 20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M 10,10 L 20,20,",
            "                ^",
            &vec![moveto(10.0, 10.0), lineto(20.0, 20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn horizontal_lineto_args() {
        test_parser(
            "M10-20H",
            "       ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20H,",
            "       ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10-20H30,",
            "          ^",
            &vec![moveto(10.0, -20.0), lineto(30.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn vertical_lineto_args() {
        test_parser(
            "M10-20v",
            "       ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20v,",
            "       ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10-20v30,",
            "          ^",
            &vec![moveto(10.0, -20.0), lineto(10.0, 10.0)],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn curveto_args() {
        test_parser(
            "M10-20C1",
            "        ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20C1 2",
            "          ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,",
            "           ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20C1 2 3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,3,",
            "             ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20C1 2 3 4",
            "              ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,3,4",
            "              ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,3,4,",
            "               ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20C1 2 3 4 5",
            "                ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,3,4,5",
            "                ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20C1,2,3,4,5,",
            "                 ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20C1,2,3,4,5,6,",
            "                   ^",
            &vec![moveto(10.0, -20.0), curveto(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn smooth_curveto_args() {
        test_parser(
            "M10-20S1",
            "        ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20S1,",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20S1 2",
            "          ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20S1,2,",
            "           ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20S1 2 3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20S1,2,3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20S1,2,3,",
            "             ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20S1,2,3,4,",
            "               ^",
            &vec![
                moveto(10.0, -20.0),
                curveto(10.0, -20.0, 1.0, 2.0, 3.0, 4.0),
            ],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn quadratic_bezier_curveto_args() {
        test_parser(
            "M10-20Q1",
            "        ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20Q1,",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20Q1 2",
            "          ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20Q1,2,",
            "           ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20Q1 2 3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20Q1,2,3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20Q1,2,3,",
            "             ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10 20 Q30 40 50 60,",
            "                    ^",
            &vec![
                moveto(10.0, 20.0),
                curveto(
                    70.0 / 3.0,
                    100.0 / 3.0,
                    110.0 / 3.0,
                    140.0 / 3.0,
                    50.0,
                    60.0,
                ),
            ],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn smooth_quadratic_bezier_curveto_args() {
        test_parser(
            "M10-20T1",
            "        ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20T1,",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10 20 T30 40,",
            "              ^",
            &vec![
                moveto(10.0, 20.0),
                curveto(10.0, 20.0, 50.0 / 3.0, 80.0 / 3.0, 30.0, 40.0),
            ],
            Some(ErrorKind::UnexpectedEof),
        );
    }

    #[test]
    fn elliptical_arc_args() {
        test_parser(
            "M10-20A1",
            "        ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20A1,",
            "         ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20A1 2",
            "          ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20A1 2,",
            "           ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20A1 2 3",
            "            ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20A1 2 3,",
            "             ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20A1 2 3 4",
            "             ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10-20A1 2 3 1",
            "              ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20A1 2 3,1,",
            "               ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20A1 2 3 1 5",
            "               ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedToken),
        );

        test_parser(
            "M10-20A1 2 3 1 1",
            "                ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20A1 2 3,1,1,",
            "                 ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        test_parser(
            "M10-20A1 2 3 1 1 6",
            "                  ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );
        test_parser(
            "M10-20A1 2 3,1,1,6,",
            "                   ^",
            &vec![moveto(10.0, -20.0)],
            Some(ErrorKind::UnexpectedEof),
        );

        // FIXME: we need tests for arcs
        //
        // test_parser("M10-20A1 2 3,1,1,6,7,",
        //             "                     ^",
        //             &vec![moveto(10.0, -20.0)
        //                   arc(...)],
        //             Some(ErrorKind::UnexpectedEof));
    }

    #[test]
    fn bugs() {
        // https://gitlab.gnome.org/GNOME/librsvg/issues/345
        test_parser(
            "M.. 1,0 0,100000",
            "  ^",
            &vec![],
            Some(ErrorKind::UnexpectedToken),
        );
    }
}
