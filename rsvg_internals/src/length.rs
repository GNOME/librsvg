use cssparser::{Parser, Token};
use std::f64::consts::*;

use crate::drawing_ctx::ViewParams;
use crate::error::*;
use crate::parsers::Parse;
use crate::parsers::{ParseError, finite_f32};
use crate::properties::ComputedValues;

pub type RsvgLength = Length;

// Keep this in sync with rsvg.h:RsvgUnit

/// Units for length values
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthUnit {
    /// `1.0` means 100%
    Percent,

    /// Pixels, or the CSS default unit
    Px,

    /// Size of the current font
    Em,

    /// x-height of the current font
    Ex,

    /// Inches (25.4 mm)
    In,

    /// Centimeters
    Cm,

    /// Millimeters
    Mm,

    /// Points (1/72 inch)
    Pt,

    /// Picas (12 points)
    Pc,
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum LengthDir {
    Horizontal,
    Vertical,
    Both,
}

impl LengthDir {
    fn scaling_factor(&self, x: f64, y: f64) -> f64 {
        match *self {
            LengthDir::Horizontal => x,
            LengthDir::Vertical => y,
            LengthDir::Both => viewport_percentage(x, y),
        }
    }
}

macro_rules! define_length_type {
    ($name:ident, $dir:expr) => {
        #[derive(Debug, PartialEq, Copy, Clone)]
        pub struct $name(Length);

        impl $name {
            pub fn new(length: f64, unit: LengthUnit) -> Self {
                $name(Length::new(length, unit))
            }

            pub fn length(&self) -> f64 {
                self.0.length
            }

            pub fn unit(&self) -> LengthUnit {
                self.0.unit
            }

            pub fn get_unitless(&self) -> f64 {
                self.0.get_unitless()
            }

            pub fn to_length(&self) -> Length {
                self.0
            }

            pub fn check_nonnegative(self) -> Result<Self, ValueErrorKind> {
                if self.length() >= 0.0 {
                    Ok(self)
                } else {
                    Err(ValueErrorKind::Value(
                        "value must be non-negative".to_string(),
                    ))
                }
            }

            pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
                match self.unit() {
                    LengthUnit::Px => self.length(),

                    LengthUnit::Percent => {
                        self.length()
                            * $dir.scaling_factor(params.view_box_width, params.view_box_height)
                    }

                    LengthUnit::Em => self.length() * font_size_from_values(values, params),

                    LengthUnit::Ex => self.length() * font_size_from_values(values, params) / 2.0,

                    LengthUnit::In => {
                        self.length() * $dir.scaling_factor(params.dpi_x, params.dpi_y)
                    }

                    LengthUnit::Cm => {
                        self.length() * $dir.scaling_factor(params.dpi_x, params.dpi_y)
                            / CM_PER_INCH
                    }

                    LengthUnit::Mm => {
                        self.length() * $dir.scaling_factor(params.dpi_x, params.dpi_y)
                            / MM_PER_INCH
                    }

                    LengthUnit::Pt => {
                        self.length() * $dir.scaling_factor(params.dpi_x, params.dpi_y)
                            / POINTS_PER_INCH
                    }

                    LengthUnit::Pc => {
                        self.length() * $dir.scaling_factor(params.dpi_x, params.dpi_y)
                            / PICA_PER_INCH
                    }
                }
            }

            pub fn from_cssparser(parser: &mut Parser<'_, '_>) -> Result<Self, ValueErrorKind> {
                Ok($name(Length::from_cssparser(parser)?))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name(Length::new(0.0, LengthUnit::Px))
            }
        }

        impl Parse for $name {
            type Err = ValueErrorKind;

            fn parse(parser: &mut Parser<'_, '_>) -> Result<$name, ValueErrorKind> {
                Ok($name(Length::parse(parser)?))
            }
        }
    };
}

// Horizontal length
//
// When this is specified as a percent value, it will get resolved
// against the current viewport's width.
define_length_type!(LengthHorizontal, LengthDir::Horizontal);

// Vertical length
//
// When this is specified as a percent value, it will get resolved
// against the current viewport's height.
define_length_type!(LengthVertical, LengthDir::Vertical);

// "Both" length
//
// When this is specified as a percent value, it will get resolved
// against the current viewport's width and height.
define_length_type!(LengthBoth, LengthDir::Both);

// Keep this in sync with rsvg.h:RsvgLength
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Length {
    pub length: f64,
    pub unit: LengthUnit,
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
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Length, ValueErrorKind> {
        Length::from_cssparser(parser)
    }
}

impl Length {
    pub fn new(l: f64, unit: LengthUnit) -> Length {
        Length { length: l, unit }
    }

    /// Returns the raw length after asserting units are either default or percent.
    #[inline]
    pub fn get_unitless(&self) -> f64 {
        assert!(self.unit == LengthUnit::Px || self.unit == LengthUnit::Percent);
        self.length
    }

    pub fn from_cssparser(parser: &mut Parser<'_, '_>) -> Result<Length, ValueErrorKind> {
        let length = {
            let token = parser.next().map_err(|_| {
                ValueErrorKind::Parse(ParseError::new(
                    "expected number and optional symbol, or number and percentage",
                ))
            })?;

            match *token {
                Token::Number { value, .. } => Length {
                    length: f64::from(finite_f32(value)?),
                    unit: LengthUnit::Px,
                },

                Token::Percentage { unit_value, .. } => Length {
                    length: f64::from(finite_f32(unit_value)?),
                    unit: LengthUnit::Percent,
                },

                Token::Dimension {
                    value, ref unit, ..
                } => {
                    let value = f64::from(finite_f32(value)?);

                    match unit.as_ref() {
                        "px" => Length {
                            length: value,
                            unit: LengthUnit::Px,
                        },

                        "em" => Length {
                            length: value,
                            unit: LengthUnit::Em,
                        },

                        "ex" => Length {
                            length: value,
                            unit: LengthUnit::Ex,
                        },

                        "in" => Length {
                            length: value,
                            unit: LengthUnit::In,
                        },

                        "cm" => Length {
                            length: value,
                            unit: LengthUnit::Cm,
                        },

                        "mm" => Length {
                            length: value,
                            unit: LengthUnit::Mm,
                        },

                        "pt" => Length {
                            length: value,
                            unit: LengthUnit::Pt,
                        },

                        "pc" => Length {
                            length: value,
                            unit: LengthUnit::Pc,
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

fn font_size_from_values(values: &ComputedValues, params: &ViewParams) -> f64 {
    let v = &values.font_size.0.value().0;

    match v.unit {
        LengthUnit::Percent => unreachable!("ComputedValues can't have a relative font size"),

        LengthUnit::Px => v.length,

        // This is the same default as used in NodeSvg::get_size()
        LengthUnit::Em => v.length * 12.0,

        // This is the same default as used in NodeSvg::get_size()
        LengthUnit::Ex => v.length * 12.0 / 2.0,

        // FontSize always is a LengthDir::Both, per properties.rs
        LengthUnit::In => v.length * LengthDir::Both.scaling_factor(params.dpi_x, params.dpi_y),
        LengthUnit::Cm => {
            v.length * LengthDir::Both.scaling_factor(params.dpi_x, params.dpi_y) / CM_PER_INCH
        }
        LengthUnit::Mm => {
            v.length * LengthDir::Both.scaling_factor(params.dpi_x, params.dpi_y) / MM_PER_INCH
        }
        LengthUnit::Pt => {
            v.length * LengthDir::Both.scaling_factor(params.dpi_x, params.dpi_y) / POINTS_PER_INCH
        }
        LengthUnit::Pc => {
            v.length * LengthDir::Both.scaling_factor(params.dpi_x, params.dpi_y) / PICA_PER_INCH
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
    Array(Vec<LengthBoth>),
}

impl Default for Dasharray {
    fn default() -> Dasharray {
        Dasharray::None
    }
}

impl Parse for Dasharray {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Dasharray, ValueErrorKind> {
        if parser
            .try_parse(|p| p.expect_ident_matching("none"))
            .is_ok()
        {
            Ok(Dasharray::None)
        } else {
            Ok(Dasharray::Array(parse_dash_array(parser)?))
        }
    }
}

// This does not handle "inherit" or "none" state, the caller is responsible for that.
fn parse_dash_array(parser: &mut Parser<'_, '_>) -> Result<Vec<LengthBoth>, ValueErrorKind> {
    let mut dasharray = Vec::new();

    loop {
        dasharray.push(LengthBoth::from_cssparser(parser).and_then(LengthBoth::check_nonnegative)?);

        if parser.is_exhausted() {
            break;
        } else if parser.try_parse(|p| p.expect_comma()).is_ok() {
            continue;
        }
    }

    Ok(dasharray)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::float_eq_cairo::ApproxEqCairo;

    #[test]
    fn parses_default() {
        assert_eq!(
            LengthHorizontal::parse_str("42"),
            Ok(LengthHorizontal(Length::new(42.0, LengthUnit::Px)))
        );

        assert_eq!(
            LengthHorizontal::parse_str("-42px"),
            Ok(LengthHorizontal(Length::new(-42.0, LengthUnit::Px)))
        );
    }

    #[test]
    fn parses_percent() {
        assert_eq!(
            LengthHorizontal::parse_str("50.0%"),
            Ok(LengthHorizontal(Length::new(0.5, LengthUnit::Percent)))
        );
    }

    #[test]
    fn parses_font_em() {
        assert_eq!(
            LengthVertical::parse_str("22.5em"),
            Ok(LengthVertical(Length::new(22.5, LengthUnit::Em)))
        );
    }

    #[test]
    fn parses_font_ex() {
        assert_eq!(
            LengthVertical::parse_str("22.5ex"),
            Ok(LengthVertical(Length::new(22.5, LengthUnit::Ex)))
        );
    }

    #[test]
    fn parses_physical_units() {
        assert_eq!(
            LengthBoth::parse_str("72pt"),
            Ok(LengthBoth(Length::new(72.0, LengthUnit::Pt)))
        );

        assert_eq!(
            LengthBoth::parse_str("-22.5in"),
            Ok(LengthBoth(Length::new(-22.5, LengthUnit::In)))
        );

        assert_eq!(
            LengthBoth::parse_str("-254cm"),
            Ok(LengthBoth(Length::new(-254.0, LengthUnit::Cm)))
        );

        assert_eq!(
            LengthBoth::parse_str("254mm"),
            Ok(LengthBoth(Length::new(254.0, LengthUnit::Mm)))
        );

        assert_eq!(
            LengthBoth::parse_str("60pc"),
            Ok(LengthBoth(Length::new(60.0, LengthUnit::Pc)))
        );
    }

    #[test]
    fn empty_length_yields_error() {
        assert!(is_parse_error(&LengthBoth::parse_str("")));
    }

    #[test]
    fn invalid_unit_yields_error() {
        assert!(is_parse_error(&LengthBoth::parse_str("8furlong")));
    }

    #[test]
    fn check_nonnegative_works() {
        assert!(LengthBoth::parse_str("0")
            .and_then(|l| l.check_nonnegative())
            .is_ok());
        assert!(LengthBoth::parse_str("-10")
            .and_then(|l| l.check_nonnegative())
            .is_err());
    }

    #[test]
    fn normalize_default_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            LengthBoth::new(10.0, LengthUnit::Px).normalize(&values, &params),
            10.0
        );
    }

    #[test]
    fn normalize_absolute_units_works() {
        let params = ViewParams::new(40.0, 50.0, 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            LengthHorizontal::new(10.0, LengthUnit::In).normalize(&values, &params),
            400.0
        );
        assert_approx_eq_cairo!(
            LengthVertical::new(10.0, LengthUnit::In).normalize(&values, &params),
            500.0
        );

        assert_approx_eq_cairo!(
            LengthHorizontal::new(10.0, LengthUnit::Cm).normalize(&values, &params),
            400.0 / CM_PER_INCH
        );
        assert_approx_eq_cairo!(
            LengthHorizontal::new(10.0, LengthUnit::Mm).normalize(&values, &params),
            400.0 / MM_PER_INCH
        );
        assert_approx_eq_cairo!(
            LengthHorizontal::new(10.0, LengthUnit::Pt).normalize(&values, &params),
            400.0 / POINTS_PER_INCH
        );
        assert_approx_eq_cairo!(
            LengthHorizontal::new(10.0, LengthUnit::Pc).normalize(&values, &params),
            400.0 / PICA_PER_INCH
        );
    }

    #[test]
    fn normalize_percent_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 200.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            LengthHorizontal::new(0.05, LengthUnit::Percent).normalize(&values, &params),
            5.0
        );
        assert_approx_eq_cairo!(
            LengthVertical::new(0.05, LengthUnit::Percent).normalize(&values, &params),
            10.0
        );
    }

    #[test]
    fn normalize_font_em_ex_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 200.0);

        let values = ComputedValues::default();

        // These correspond to the default size for the font-size
        // property and the way we compute Em/Ex from that.

        assert_approx_eq_cairo!(
            LengthVertical::new(1.0, LengthUnit::Em).normalize(&values, &params),
            12.0
        );

        assert_approx_eq_cairo!(
            LengthVertical::new(1.0, LengthUnit::Ex).normalize(&values, &params),
            6.0
        );
    }

    fn parse_dash_array_str(s: &str) -> Result<Dasharray, ValueErrorKind> {
        Dasharray::parse_str(s)
    }

    #[test]
    fn parses_dash_array() {
        // helper to cut down boilderplate
        let length_parse = |s| LengthBoth::parse_str(s).unwrap();

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
