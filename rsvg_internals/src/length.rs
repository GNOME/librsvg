use cssparser::{Parser, Token};
use std::f64::consts::*;

use drawing_ctx::ViewParams;
use error::*;
use parsers::Parse;
use parsers::ParseError;
use state::ComputedValues;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthUnit {
    Default,
    Percent,
    FontEm,
    FontEx,
    Inch,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthDir {
    Horizontal,
    Vertical,
    Both,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Length {
    pub length: f64,
    pub unit: LengthUnit,
    dir: LengthDir,
}

impl Default for Length {
    fn default() -> Length {
        Length {
            length: 0.0,
            unit: LengthUnit::Default,
            dir: LengthDir::Both,
        }
    }
}

pub const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH: f64 = 2.54;
const MM_PER_INCH: f64 = 25.4;
const PICA_PER_INCH: f64 = 6.0;

// https://www.w3.org/TR/SVG/types.html#DataTypeLength
// https://www.w3.org/TR/2008/REC-CSS2-20080411/syndata.html#length-units
// Lengths have units.  When they need to be need resolved to
// units in the user's coordinate system, some unit types
// need to know if they are horizontal/vertical/both.  For example,
// a some_object.width="50%" is 50% with respect to the current
// viewport's width.  In this case, the @dir argument is used
// inside Length::normalize(), when it needs to know to what the
// length refers.

fn make_err() -> ValueErrorKind {
    ValueErrorKind::Parse(ParseError::new(
        "expected length: number(\"em\" | \"ex\" | \"px\" | \"in\" | \"cm\" | \"mm\" | \"pt\" | \
         \"pc\" | \"%\")?",
    ))
}

impl Parse for Length {
    type Data = LengthDir;
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, dir: LengthDir) -> Result<Length, ValueErrorKind> {
        let length = Length::from_cssparser(parser, dir)?;

        parser.expect_exhausted().map_err(|_| make_err())?;

        Ok(length)
    }
}

impl Length {
    pub fn new(l: f64, unit: LengthUnit, dir: LengthDir) -> Length {
        Length {
            length: l,
            unit,
            dir,
        }
    }

    pub fn check_nonnegative(self) -> Result<Length, ValueErrorKind> {
        if self.length >= 0.0 {
            Ok(self)
        } else {
            Err(ValueErrorKind::Value(
                "value must be non-negative".to_string(),
            ))
        }
    }

    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        match self.unit {
            LengthUnit::Default => self.length,

            LengthUnit::Percent => match self.dir {
                LengthDir::Horizontal => self.length * params.view_box_width(),
                LengthDir::Vertical => self.length * params.view_box_height(),
                LengthDir::Both => {
                    self.length
                        * viewport_percentage(params.view_box_width(), params.view_box_height())
                }
            },

            LengthUnit::FontEm => self.length * font_size_from_values(values, params),

            LengthUnit::FontEx => self.length * font_size_from_values(values, params) / 2.0,

            LengthUnit::Inch => font_size_from_inch(self.length, self.dir, params),
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
        }
    }

    pub fn from_cssparser(
        parser: &mut Parser<'_, '_>,
        dir: LengthDir,
    ) -> Result<Length, ValueErrorKind> {
        let length = {
            let token = parser.next().map_err(|_| {
                ValueErrorKind::Parse(ParseError::new(
                    "expected number and optional symbol, or number and percentage",
                ))
            })?;

            match *token {
                Token::Number { value, .. } => Length {
                    length: f64::from(value),
                    unit: LengthUnit::Default,
                    dir,
                },

                Token::Percentage { unit_value, .. } => Length {
                    length: f64::from(unit_value),
                    unit: LengthUnit::Percent,
                    dir,
                },

                Token::Dimension {
                    value, ref unit, ..
                } => {
                    let value = f64::from(value);

                    match unit.as_ref() {
                        "em" => Length {
                            length: value,
                            unit: LengthUnit::FontEm,
                            dir,
                        },

                        "ex" => Length {
                            length: value,
                            unit: LengthUnit::FontEx,
                            dir,
                        },

                        "pt" => Length {
                            length: value / POINTS_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "in" => Length {
                            length: value,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "cm" => Length {
                            length: value / CM_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "mm" => Length {
                            length: value / MM_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "pc" => Length {
                            length: value / PICA_PER_INCH,
                            unit: LengthUnit::Inch,
                            dir,
                        },

                        "px" => Length {
                            length: value,
                            unit: LengthUnit::Default,
                            dir,
                        },

                        _ => return Err(make_err()),
                    }
                }

                _ => return Err(make_err()),
            }
        };

        Ok(length)
    }
}

fn font_size_from_inch(length: f64, dir: LengthDir, params: &ViewParams) -> f64 {
    match dir {
        LengthDir::Horizontal => length * params.dpi_x(),
        LengthDir::Vertical => length * params.dpi_y(),
        LengthDir::Both => length * viewport_percentage(params.dpi_x(), params.dpi_y()),
    }
}

fn font_size_from_values(values: &ComputedValues, params: &ViewParams) -> f64 {
    let v = &values.font_size.0.value();

    match v.unit {
        LengthUnit::Default => v.length,

        LengthUnit::Inch => font_size_from_inch(v.length, v.dir, params),

        LengthUnit::Percent => unreachable!("ComputedValues can't have a relative font size"),

        LengthUnit::FontEm | LengthUnit::FontEx => {
            // This is the same default as used in rsvg_node_svg_get_size()
            v.hand_normalize(0.0, 0.0, 12.0)
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
    Array(Vec<Length>),
}

impl Default for Dasharray {
    fn default() -> Dasharray {
        Dasharray::None
    }
}

impl Parse for Dasharray {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: Self::Data) -> Result<Dasharray, ValueErrorKind> {
        if parser.try(|p| p.expect_ident_matching("none")).is_ok() {
            Ok(Dasharray::None)
        } else {
            Ok(Dasharray::Array(parse_dash_array(parser)?))
        }
    }
}

// This does not handle "inherit" or "none" state, the caller is responsible for that.
fn parse_dash_array(parser: &mut Parser<'_, '_>) -> Result<Vec<Length>, ValueErrorKind> {
    let mut dasharray = Vec::new();

    loop {
        dasharray.push(
            Length::from_cssparser(parser, LengthDir::Both).and_then(Length::check_nonnegative)?,
        );

        if parser.is_exhausted() {
            break;
        } else if parser.try(|p| p.expect_comma()).is_ok() {
            continue;
        }
    }

    Ok(dasharray)
}

#[cfg(test)]
mod tests {
    use super::*;

    use float_eq_cairo::ApproxEqCairo;

    #[test]
    fn parses_default() {
        assert_eq!(
            Length::parse_str("42", LengthDir::Horizontal),
            Ok(Length::new(
                42.0,
                LengthUnit::Default,
                LengthDir::Horizontal
            ))
        );

        assert_eq!(
            Length::parse_str("-42px", LengthDir::Horizontal),
            Ok(Length::new(
                -42.0,
                LengthUnit::Default,
                LengthDir::Horizontal
            ))
        );
    }

    #[test]
    fn parses_percent() {
        assert_eq!(
            Length::parse_str("50.0%", LengthDir::Horizontal),
            Ok(Length::new(0.5, LengthUnit::Percent, LengthDir::Horizontal))
        );
    }

    #[test]
    fn parses_font_em() {
        assert_eq!(
            Length::parse_str("22.5em", LengthDir::Vertical),
            Ok(Length::new(22.5, LengthUnit::FontEm, LengthDir::Vertical))
        );
    }

    #[test]
    fn parses_font_ex() {
        assert_eq!(
            Length::parse_str("22.5ex", LengthDir::Vertical),
            Ok(Length::new(22.5, LengthUnit::FontEx, LengthDir::Vertical))
        );
    }

    #[test]
    fn parses_physical_units() {
        assert_eq!(
            Length::parse_str("72pt", LengthDir::Both),
            Ok(Length::new(1.0, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            Length::parse_str("-22.5in", LengthDir::Both),
            Ok(Length::new(-22.5, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            Length::parse_str("-254cm", LengthDir::Both),
            Ok(Length::new(-100.0, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            Length::parse_str("254mm", LengthDir::Both),
            Ok(Length::new(10.0, LengthUnit::Inch, LengthDir::Both))
        );

        assert_eq!(
            Length::parse_str("60pc", LengthDir::Both),
            Ok(Length::new(10.0, LengthUnit::Inch, LengthDir::Both))
        );
    }

    #[test]
    fn empty_length_yields_error() {
        assert!(is_parse_error(&Length::parse_str("", LengthDir::Both)));
    }

    #[test]
    fn invalid_unit_yields_error() {
        assert!(is_parse_error(&Length::parse_str(
            "8furlong",
            LengthDir::Both
        )));
    }

    #[test]
    fn check_nonnegative_works() {
        assert!(Length::parse_str("0", LengthDir::Both)
            .and_then(|l| l.check_nonnegative())
            .is_ok());
        assert!(Length::parse_str("-10", LengthDir::Both)
            .and_then(|l| l.check_nonnegative())
            .is_err());
    }

    #[test]
    fn normalize_default_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::new(10.0, LengthUnit::Default, LengthDir::Both).normalize(&values, &params),
            10.0
        );
    }

    #[test]
    fn normalize_absolute_units_works() {
        let params = ViewParams::new(40.0, 50.0, 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::new(10.0, LengthUnit::Inch, LengthDir::Horizontal).normalize(&values, &params),
            400.0
        );
        assert_approx_eq_cairo!(
            Length::new(10.0, LengthUnit::Inch, LengthDir::Vertical).normalize(&values, &params),
            500.0
        );
    }

    #[test]
    fn normalize_percent_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 200.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::new(0.05, LengthUnit::Percent, LengthDir::Horizontal)
                .normalize(&values, &params),
            5.0
        );
        assert_approx_eq_cairo!(
            Length::new(0.05, LengthUnit::Percent, LengthDir::Vertical).normalize(&values, &params),
            10.0
        );
    }

    #[test]
    fn normalize_font_em_ex_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 200.0);

        let values = ComputedValues::default();

        // These correspond to the default size for the font-size
        // property and the way we compute FontEx from that.

        assert_approx_eq_cairo!(
            Length::new(1.0, LengthUnit::FontEm, LengthDir::Vertical).normalize(&values, &params),
            12.0
        );

        assert_approx_eq_cairo!(
            Length::new(1.0, LengthUnit::FontEx, LengthDir::Vertical).normalize(&values, &params),
            6.0
        );
    }

    fn parse_dash_array_str(s: &str) -> Result<Dasharray, ValueErrorKind> {
        Dasharray::parse_str(s, ())
    }

    #[test]
    fn parses_dash_array() {
        // helper to cut down boilderplate
        let length_parse = |s| Length::parse_str(s, LengthDir::Both).unwrap();

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
            Err(ValueErrorKind::Value(String::from(
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
