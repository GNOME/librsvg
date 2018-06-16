use cssparser::{Parser, Token};
use libc;
use std::f64::consts::*;

use drawing_ctx::{DrawingCtx, RsvgDrawingCtx};
use error::*;
use parsers::Parse;
use parsers::ParseError;
use state::{ComputedValues, RsvgComputedValues};
use util::utf8_cstr;

// Keep this in sync with ../../rsvg-private.h:LengthUnit
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthUnit {
    Default,
    Percent,
    FontEm,
    FontEx,
    Inch,
    RelativeLarger,
    RelativeSmaller,
}

// Keep this in sync with ../../rsvg-private.h:LengthDir
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthDir {
    Horizontal,
    Vertical,
    Both,
}

// This is *not* an opaque struct; it is actually visible to the C code.  It is so
// that the remaining C code can create RsvgLength values as part of existing
// structures or objects, without allocations on the heap.
//
// Keep this in sync with ../../rsvg-private.h:RsvgLength
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct RsvgLength {
    pub length: f64,
    pub unit: LengthUnit,
    dir: LengthDir,
}

impl Default for RsvgLength {
    fn default() -> RsvgLength {
        RsvgLength {
            length: 0.0,
            unit: LengthUnit::Default,
            dir: LengthDir::Both,
        }
    }
}

const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH: f64 = 2.54;
const MM_PER_INCH: f64 = 25.4;
const PICA_PER_INCH: f64 = 6.0;

fn compute_named_size(name: &str) -> f64 {
    let power: f64;

    match name {
        "xx-small" => {
            power = -3.0;
        }
        "x-small" => {
            power = -2.0;
        }
        "small" => {
            power = -1.0;
        }
        "medium" => {
            power = 0.0;
        }
        "large" => {
            power = 1.0;
        }
        "x-large" => {
            power = 2.0;
        }
        "xx-large" => {
            power = 3.0;
        }
        _ => {
            unreachable!();
        }
    }

    12.0 * 1.2f64.powf(power) / POINTS_PER_INCH
}

#[no_mangle]
pub extern "C" fn rsvg_length_parse(string: *const libc::c_char, dir: LengthDir) -> RsvgLength {
    let my_string = unsafe { utf8_cstr(string) };

    // FIXME: this ignores errors; propagate them upstream
    RsvgLength::parse_str(my_string, dir).unwrap_or_else(|_| RsvgLength::default())
}

// https://www.w3.org/TR/SVG/types.html#DataTypeLength
// https://www.w3.org/TR/2008/REC-CSS2-20080411/syndata.html#length-units
// Lengths have units.  When they need to be need resolved to
// units in the user's coordinate system, some unit types
// need to know if they are horizontal/vertical/both.  For example,
// a some_object.width="50%" is 50% with respect to the current
// viewport's width.  In this case, the @dir argument is used
// inside RsvgLength::normalize(), when it needs to know to what the
// length refers.

fn make_err() -> AttributeError {
    AttributeError::Parse(ParseError::new(
        "expected length: number(\"em\" | \"ex\" | \"px\" | \"in\" | \"cm\" | \"mm\" | \"pt\" | \
         \"pc\" | \"%\")?",
    ))
}

impl Parse for RsvgLength {
    type Data = LengthDir;
    type Err = AttributeError;

    fn parse(parser: &mut Parser, dir: LengthDir) -> Result<RsvgLength, AttributeError> {
        let length = RsvgLength::from_cssparser(parser, dir)?;

        parser.expect_exhausted().map_err(|_| make_err())?;

        Ok(length)
    }
}

impl RsvgLength {
    pub fn new(l: f64, unit: LengthUnit, dir: LengthDir) -> RsvgLength {
        RsvgLength {
            length: l,
            unit,
            dir,
        }
    }

    pub fn check_nonnegative(self) -> Result<RsvgLength, AttributeError> {
        if self.length >= 0.0 {
            Ok(self)
        } else {
            Err(AttributeError::Value(
                "value must be non-negative".to_string(),
            ))
        }
    }

    pub fn normalize(&self, values: &ComputedValues, draw_ctx: &DrawingCtx) -> f64 {
        match self.unit {
            LengthUnit::Default => self.length,

            LengthUnit::Percent => {
                let (width, height) = draw_ctx.get_view_box_size();

                match self.dir {
                    LengthDir::Horizontal => self.length * width,
                    LengthDir::Vertical => self.length * height,
                    LengthDir::Both => self.length * viewport_percentage(width, height),
                }
            }

            LengthUnit::FontEm => self.length * font_size_from_values(values, draw_ctx),

            LengthUnit::FontEx => self.length * font_size_from_values(values, draw_ctx) / 2.0,

            LengthUnit::Inch => font_size_from_inch(self.length, self.dir, draw_ctx),

            LengthUnit::RelativeLarger => font_size_from_values(values, draw_ctx) * 1.2,

            LengthUnit::RelativeSmaller => font_size_from_values(values, draw_ctx) / 1.2,
        }
    }

    /// Returns the raw length after asserting units are either default or percent.
    #[inline]
    pub fn get_unitless(&self) -> f64 {
        assert!(self.unit == LengthUnit::Default || self.unit == LengthUnit::Percent);
        self.length
    }

    pub fn hand_normalize(
        &self,
        pixels_per_inch: f64,
        width_or_height: f64,
        font_size: f64,
    ) -> f64 {
        match self.unit {
            LengthUnit::Default => self.length,

            LengthUnit::Percent => self.length * width_or_height,

            LengthUnit::FontEm => self.length * font_size,

            LengthUnit::FontEx => self.length * font_size / 2.0,

            LengthUnit::Inch => self.length * pixels_per_inch,

            _ => 0.0,
        }
    }

    pub fn from_cssparser(
        parser: &mut Parser,
        dir: LengthDir,
    ) -> Result<RsvgLength, AttributeError> {
        let length = {
            let token = parser.next().map_err(|_| {
                AttributeError::Parse(ParseError::new(
                    "expected number and optional symbol, or number and percentage",
                ))
            })?;

            match *token {
                Token::Number { value, .. } => RsvgLength {
                    length: f64::from(value),
                    unit: LengthUnit::Default,
                    dir,
                },

                Token::Percentage { unit_value, .. } => RsvgLength {
                    length: f64::from(unit_value),
                    unit: LengthUnit::Percent,
                    dir,
                },

                Token::Dimension {
                    value, ref unit, ..
                } => {
                    let value = f64::from(value);

                    match unit.as_ref() {
                        "em" => RsvgLength {
                            length: value,
                            unit: LengthUnit::FontEm,
                            dir,
                        },

                        "ex" => RsvgLength {
                            length: value,
                            unit: LengthUnit::FontEx,
                            dir,
                        },

                        "pt" => RsvgLength {
                            length: value / POINTS_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "in" => RsvgLength {
                            length: value,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "cm" => RsvgLength {
                            length: value / CM_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "mm" => RsvgLength {
                            length: value / MM_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "pc" => RsvgLength {
                            length: value / PICA_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "px" => RsvgLength {
                            length: value,
                            unit: LengthUnit::Default,
                            dir,
                        },

                        _ => return Err(make_err()),
                    }
                }

                // FIXME: why are the following in Length?  They should be in FontSize
                Token::Ident(ref cow) => match cow.as_ref() {
                    "larger" => RsvgLength {
                        length: 0.0,
                        unit: LengthUnit::RelativeLarger,
                        dir,
                    },

                    "smaller" => RsvgLength {
                        length: 0.0,
                        unit: LengthUnit::RelativeSmaller,
                        dir,
                    },

                    "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large"
                    | "xx-large" => RsvgLength {
                        length: compute_named_size(cow),
                        unit: LengthUnit::Inch,
                        dir,
                    },

                    _ => return Err(make_err()),
                },

                _ => return Err(make_err()),
            }
        };

        Ok(length)
    }
}

fn font_size_from_inch(length: f64, dir: LengthDir, draw_ctx: &DrawingCtx) -> f64 {
    let (dpi_x, dpi_y) = draw_ctx.get_dpi();

    match dir {
        LengthDir::Horizontal => length * dpi_x,
        LengthDir::Vertical => length * dpi_y,
        LengthDir::Both => length * viewport_percentage(dpi_x, dpi_y),
    }
}

fn font_size_from_values(values: &ComputedValues, draw_ctx: &DrawingCtx) -> f64 {
    let v = &values.font_size.0;

    match v.unit {
        LengthUnit::Default => v.length,

        LengthUnit::Inch => font_size_from_inch(v.length, v.dir, draw_ctx),

        LengthUnit::Percent
        | LengthUnit::FontEm
        | LengthUnit::FontEx
        | LengthUnit::RelativeLarger
        | LengthUnit::RelativeSmaller => {
            unreachable!("ComputedValues can't have a relative font size")
        }
    }
}

fn viewport_percentage(x: f64, y: f64) -> f64 {
    // https://www.w3.org/TR/SVG/coords.html#Units
    // "For any other length value expressed as a percentage of the viewport, the
    // percentage is calculated as the specified percentage of
    // sqrt((actual-width)**2 + (actual-height)**2))/sqrt(2)."
    (x * x + y * y).sqrt() / SQRT_2
}

#[derive(Debug, PartialEq, Clone)]
pub enum Dasharray {
    None,
    Array(Vec<RsvgLength>),
}

impl Default for Dasharray {
    fn default() -> Dasharray {
        Dasharray::None
    }
}

impl Parse for Dasharray {
    type Data = ();
    type Err = AttributeError;

    fn parse(parser: &mut Parser, _: Self::Data) -> Result<Dasharray, AttributeError> {
        if parser.try(|p| p.expect_ident_matching("none")).is_ok() {
            Ok(Dasharray::None)
        } else {
            Ok(Dasharray::Array(parse_dash_array(parser)?))
        }
    }
}

// This does not handle "inherit" or "none" state, the caller is responsible for that.
fn parse_dash_array(parser: &mut Parser) -> Result<Vec<RsvgLength>, AttributeError> {
    let mut dasharray = Vec::new();

    loop {
        dasharray.push(
            RsvgLength::from_cssparser(parser, LengthDir::Both)
                .and_then(RsvgLength::check_nonnegative)?,
        );

        if parser.is_exhausted() {
            break;
        } else if parser.try(|p| p.expect_comma()).is_ok() {
            continue;
        }
    }

    Ok(dasharray)
}

#[no_mangle]
pub extern "C" fn rsvg_length_normalize(
    raw_length: *const RsvgLength,
    values: RsvgComputedValues,
    draw_ctx: *const RsvgDrawingCtx,
) -> f64 {
    assert!(!raw_length.is_null());
    let length: &RsvgLength = unsafe { &*raw_length };

    assert!(!values.is_null());
    let values = unsafe { &*values };

    assert!(!draw_ctx.is_null());
    let draw_ctx = unsafe { &*(draw_ctx as *const DrawingCtx) };

    length.normalize(values, draw_ctx)
}

#[no_mangle]
pub extern "C" fn rsvg_length_hand_normalize(
    raw_length: *const RsvgLength,
    pixels_per_inch: f64,
    width_or_height: f64,
    font_size: f64,
) -> f64 {
    assert!(!raw_length.is_null());

    let length: &RsvgLength = unsafe { &*raw_length };

    length.hand_normalize(pixels_per_inch, width_or_height, font_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default() {
        assert_eq!(
            RsvgLength::parse_str("42", LengthDir::Horizontal),
            Ok(RsvgLength::new(
                42.0,
                LengthUnit::Default,
                LengthDir::Horizontal
            ))
        );

        assert_eq!(
            RsvgLength::parse_str("-42px", LengthDir::Horizontal),
            Ok(RsvgLength::new(
                -42.0,
                LengthUnit::Default,
                LengthDir::Horizontal
            ))
        );
    }

    #[test]
    fn parses_percent() {
        assert_eq!(
            RsvgLength::parse_str("50.0%", LengthDir::Horizontal),
            Ok(RsvgLength::new(
                0.5,
                LengthUnit::Percent,
                LengthDir::Horizontal
            ))
        );
    }

    #[test]
    fn parses_font_em() {
        assert_eq!(
            RsvgLength::parse_str("22.5em", LengthDir::Vertical),
            Ok(RsvgLength::new(
                22.5,
                LengthUnit::FontEm,
                LengthDir::Vertical
            ))
        );
    }

    #[test]
    fn parses_font_ex() {
        assert_eq!(
            RsvgLength::parse_str("22.5ex", LengthDir::Vertical),
            Ok(RsvgLength::new(
                22.5,
                LengthUnit::FontEx,
                LengthDir::Vertical
            ))
        );
    }

    #[test]
    fn parses_physical_units() {
        assert_eq!(
            RsvgLength::parse_str("72pt", LengthDir::Both),
            Ok(RsvgLength::new(1.0, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            RsvgLength::parse_str("-22.5in", LengthDir::Both),
            Ok(RsvgLength::new(-22.5, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            RsvgLength::parse_str("-254cm", LengthDir::Both),
            Ok(RsvgLength::new(-100.0, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            RsvgLength::parse_str("254mm", LengthDir::Both),
            Ok(RsvgLength::new(10.0, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            RsvgLength::parse_str("60pc", LengthDir::Both),
            Ok(RsvgLength::new(10.0, LengthUnit::Inch, LengthDir::Both))
        );
    }

    #[test]
    fn parses_relative_larger() {
        assert_eq!(
            RsvgLength::parse_str("larger", LengthDir::Vertical),
            Ok(RsvgLength::new(
                0.0,
                LengthUnit::RelativeLarger,
                LengthDir::Vertical
            ))
        );
    }

    #[test]
    fn parses_relative_smaller() {
        assert_eq!(
            RsvgLength::parse_str("smaller", LengthDir::Vertical),
            Ok(RsvgLength::new(
                0.0,
                LengthUnit::RelativeSmaller,
                LengthDir::Vertical
            ))
        );
    }

    #[test]
    fn parses_named_sizes() {
        let names = vec![
            "xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large",
        ];

        let mut previous_value: Option<f64> = None;

        // Just ensure that the values are progressively larger; don't
        // enforce a particular sequence.

        for name in names {
            let length = RsvgLength::parse_str(name, LengthDir::Both).unwrap();

            assert_eq!(length.unit, LengthUnit::Inch);
            assert_eq!(length.dir, LengthDir::Both);

            if let Some(v) = previous_value {
                assert!(length.length > v);
            } else {
                previous_value = Some(length.length);
            }
        }
    }

    #[test]
    fn empty_length_yields_error() {
        assert!(is_parse_error(&RsvgLength::parse_str("", LengthDir::Both)));
    }

    #[test]
    fn invalid_unit_yields_error() {
        assert!(is_parse_error(&RsvgLength::parse_str(
            "8furlong",
            LengthDir::Both
        )));
    }

    #[test]
    fn invalid_font_size_yields_error() {
        // FIXME: this is intended to test the (absence of) the "larger" et al values.
        // Since they really be in FontSize, not RsvgLength, we should remember
        // to move this test to that type later.
        assert!(is_parse_error(&RsvgLength::parse_str(
            "furlong",
            LengthDir::Both
        )));
    }

    #[test]
    fn check_nonnegative_works() {
        assert!(
            RsvgLength::parse_str("0", LengthDir::Both)
                .and_then(|l| l.check_nonnegative())
                .is_ok()
        );
        assert!(
            RsvgLength::parse_str("-10", LengthDir::Both)
                .and_then(|l| l.check_nonnegative())
                .is_err()
        );
    }

    fn parse_dash_array_str(s: &str) -> Result<Dasharray, AttributeError> {
        Dasharray::parse_str(s, ())
    }

    #[test]
    fn parses_dash_array() {
        // helper to cut down boilderplate
        let length_parse = |s| RsvgLength::parse_str(s, LengthDir::Both).unwrap();

        let expected = Dasharray::Array(vec![
            length_parse("1"),
            length_parse("2in"),
            length_parse("3"),
            length_parse("4%"),
        ]);

        let sample_1 = Dasharray::Array(vec![length_parse("10"), length_parse("6")]);

        let sample_2 = Dasharray::Array(vec![
            length_parse("5"),
            length_parse("5"),
            length_parse("20"),
        ]);

        let sample_3 = Dasharray::Array(vec![
            length_parse("10px"),
            length_parse("20px"),
            length_parse("20px"),
        ]);

        let sample_4 = Dasharray::Array(vec![
            length_parse("25"),
            length_parse("5"),
            length_parse("5"),
            length_parse("5"),
        ]);

        let sample_5 = Dasharray::Array(vec![length_parse("3.1415926"), length_parse("8")]);
        let sample_6 = Dasharray::Array(vec![length_parse("5"), length_parse("3.14")]);
        let sample_7 = Dasharray::Array(vec![length_parse("2")]);

        assert_eq!(parse_dash_array_str("none").unwrap(), Dasharray::None);
        assert_eq!(parse_dash_array_str("1 2in,3 4%").unwrap(), expected);
        assert_eq!(parse_dash_array_str("10,6").unwrap(), sample_1);
        assert_eq!(parse_dash_array_str("5,5,20").unwrap(), sample_2);
        assert_eq!(parse_dash_array_str("10px 20px 20px").unwrap(), sample_3);
        assert_eq!(parse_dash_array_str("25  5 , 5 5").unwrap(), sample_4);
        assert_eq!(parse_dash_array_str("3.1415926,8").unwrap(), sample_5);
        assert_eq!(parse_dash_array_str("5, 3.14").unwrap(), sample_6);
        assert_eq!(parse_dash_array_str("2").unwrap(), sample_7);

        // Negative numbers
        assert_eq!(
            parse_dash_array_str("20,40,-20"),
            Err(AttributeError::Value(String::from(
                "value must be non-negative"
            )))
        );

        // Empty dash_array
        assert!(parse_dash_array_str("").is_err());
        assert!(parse_dash_array_str("\t  \n     ").is_err());
        assert!(parse_dash_array_str(",,,").is_err());
        assert!(parse_dash_array_str("10,  \t, 20 \n").is_err());
        // No trailing commas allowed, parse error
        assert!(parse_dash_array_str("10,").is_err());
        // A comma should be followed by a number
        assert!(parse_dash_array_str("20,,10").is_err());
    }
}
