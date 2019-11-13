//! CSS length values.
//!
//! While the actual representation of CSS lengths is in the
//! [`Length`] struct, most of librsvg's internals use the newtypes
//! [`LengthHorizontal`], [`LengthVertical`], or [`LengthBoth`] depending on
//!
//! For example, the implementation of [`Circle`] defines this structure:
//!
//! ```ignore
//! pub struct Circle {
//!     cx: LengthHorizontal,
//!     cy: LengthVertical,
//!     r: LengthBoth,
//! }
//! ```
//!
//! Here, `cx` and `cy` define the center of the circle.  If the SVG document specified them as
//! percentages (e.g. `<circle cx="50%" cy="30%">`, they would need to be resolved against the
//! current viewport's width and height, respectively; that's why those fields are of type
//! [`LengthHorizontal`] and [`LengthVertical`].
//!
//! However, `r` needs to be resolved against both dimensions of the current viewport, and so
//! it is of type [`LengthBoth`].
//!
//! [`Circle`]: ../shapes/struct.Circle.html
//! [`Length`]: struct.Length.html
//! [`LengthHorizontal`]: struct.LengthHorizontal.html
//! [`LengthVertical`]: struct.LengthVertical.html
//! [`LengthBoth`]: struct.LengthBoth.html

use cssparser::{Parser, Token};
use std::f64::consts::*;
use std::marker::PhantomData;

use crate::drawing_ctx::ViewParams;
use crate::error::*;
use crate::parsers::Parse;
use crate::parsers::{finite_f32, ParseError};
use crate::properties::ComputedValues;

/// Units for length values.
// This needs to be kept in sync with `rsvg.h:RsvgUnit`.
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

/// A CSS length value.
///
/// This is equivalent to [CSS lengths].
///
/// [CSS lengths]: https://www.w3.org/TR/CSS21/syndata.html#length-units
///
/// It is up to the calling application to convert lengths in non-pixel units
/// (i.e. those where the [`unit`] field is not [`LengthUnit::Px`]) into something
/// meaningful to the application.  For example, if your application knows the
/// dots-per-inch (DPI) it is using, it can convert lengths with [`unit`] in
/// [`LengthUnit::In`] or other physical units.
///
/// [`unit`]: #structfield.unit
/// [`LengthUnit::Px`]: enum.LengthUnit.html#variant.Px
/// [`LengthUnit::In`]: enum.LengthUnit.html#variant.In
// Keep this in sync with rsvg.h:RsvgLength
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct RsvgLength {
    /// Numeric part of the length
    pub length: f64,

    /// Unit part of the length
    pub unit: LengthUnit,
}

impl RsvgLength {
    pub fn new(l: f64, unit: LengthUnit) -> RsvgLength {
        RsvgLength { length: l, unit }
    }
}

pub trait Orientation {
    /// Computes a direction-based scaling factor.
    ///
    /// This is so that `Length<Both>` will use the "normalized diagonal length" of the current
    /// viewport, per https://www.w3.org/TR/SVG/coords.html#Units
    fn normalize(x: f64, y: f64) -> f64;
}

/// Struct to be able to declare `Length<Horizontal>`
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Horizontal;

/// Struct to be able to declare `Length<Vertical>`
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Vertical;

/// Struct to be able to declare `Length<Both>`
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Both;

impl Orientation for Horizontal {
    #[inline]
    fn normalize(x: f64, _y: f64) -> f64 {
        x
    }
}

impl Orientation for Vertical {
    #[inline]
    fn normalize(_x: f64, y: f64) -> f64 {
        y
    }
}

impl Orientation for Both {
    #[inline]
    fn normalize(x: f64, y: f64) -> f64 {
        viewport_percentage(x, y)
    }
}

/// A CSS length value.
///
/// https://www.w3.org/TR/SVG/types.html#DataTypeLength
/// https://www.w3.org/TR/2008/REC-CSS2-20080411/syndata.html#length-units
///
/// Length values need to know whether they will be normalized with respect to the width,
/// height, or both dimensions of the current viewport.  So, a `Length` has a type parameter
/// [`Orientation`].  We provide [`Horizontal`], [`Vertical`], and [`Both`] implementations of
/// [`Orientation`]; these let length values know how to normalize themselves with respect to
/// the current viewport.
///
/// [`Orientation`]: trait.Orientation.html
/// [`Horizontal`]: struct.Horizontal.html
/// [`Vertical`]: struct.Vertical.html
/// [`Both`]: struct.Both.html

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Length<O: Orientation> {
    /// Numeric part of the length
    pub length: f64,

    /// Unit part of the length
    pub unit: LengthUnit,

    orientation: PhantomData<O>,
}

impl<O: Orientation> From<Length<O>> for RsvgLength {
    fn from(l: Length<O>) -> RsvgLength {
        RsvgLength {
            length: l.length,
            unit: l.unit,
        }
    }
}

impl<O: Orientation> Default for Length<O> {
    fn default() -> Self {
        Length::new(0.0, LengthUnit::Px)
    }
}

pub const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH: f64 = 2.54;
const MM_PER_INCH: f64 = 25.4;
const PICA_PER_INCH: f64 = 6.0;

fn make_err() -> ValueErrorKind {
    ValueErrorKind::Parse(ParseError::new(
        "expected length: number(\"em\" | \"ex\" | \"px\" | \"in\" | \"cm\" | \"mm\" | \"pt\" | \
         \"pc\" | \"%\")?",
    ))
}

impl<O: Orientation> Parse for Length<O> {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Length<O>, ValueErrorKind> {
        let length = {
            let token = parser.next().map_err(|_| {
                ValueErrorKind::Parse(ParseError::new(
                    "expected number and optional symbol, or number and percentage",
                ))
            })?;

            match *token {
                Token::Number { value, .. } => Length::new(
                    f64::from(finite_f32(value)?),
                    LengthUnit::Px,
                ),

                Token::Percentage { unit_value, .. } => Length::new(
                    f64::from(finite_f32(unit_value)?),
                    LengthUnit::Percent,
                ),

                Token::Dimension {
                    value, ref unit, ..
                } => {
                    let value = f64::from(finite_f32(value)?);

                    match unit.as_ref() {
                        "px" => Length::new(value, LengthUnit::Px),
                        "em" => Length::new(value, LengthUnit::Em),
                        "ex" => Length::new(value, LengthUnit::Ex),
                        "in" => Length::new(value, LengthUnit::In),
                        "cm" => Length::new(value, LengthUnit::Cm),
                        "mm" => Length::new(value, LengthUnit::Mm),
                        "pt" => Length::new(value, LengthUnit::Pt),
                        "pc" => Length::new(value, LengthUnit::Pc),

                        _ => return Err(make_err()),
                    }
                }

                _ => return Err(make_err()),
            }
        };

        Ok(length)
    }
}

impl<O: Orientation> Length<O> {
    pub fn new(l: f64, unit: LengthUnit) -> Length<O> {
        Length {
            length: l,
            unit,
            orientation: PhantomData,
        }
    }

    /// Returns `self` if the length is >= 0, or an error.
    ///
    /// ```ignore
    /// let mut parser = Parser::new(...);
    ///
    /// let length = LENGTH::parse(&mut parser).and_then($name::check_nonnegative)?;
    /// ```
    pub fn check_nonnegative(self) -> Result<Self, ValueErrorKind> {
        if self.length >= 0.0 {
            Ok(self)
        } else {
            Err(ValueErrorKind::Value(
                "value must be non-negative".to_string(),
            ))
        }
    }

    /// Normalizes a specified length into a used value.
    ///
    /// Lengths may come with non-pixel units, and when rendering, they need to be
    /// normalized to pixels based on the current viewport (e.g. for lengths with
    /// percent units), and on the current element's set of `ComputedValues` (e.g. for
    /// lengths with `Em` units that need to be resolved against the current font
    /// size).
    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        match self.unit {
            LengthUnit::Px => self.length,

            LengthUnit::Percent => {
                self.length 
                    * <O as Orientation>::normalize(params.view_box_width, params.view_box_height)
            }

            LengthUnit::Em => self.length * font_size_from_values(values, params),

            LengthUnit::Ex => self.length * font_size_from_values(values, params) / 2.0,

            LengthUnit::In => {
                self.length * <O as Orientation>::normalize(params.dpi_x, params.dpi_y)
            }

            LengthUnit::Cm => {
                self.length * <O as Orientation>::normalize(params.dpi_x, params.dpi_y)
                    / CM_PER_INCH
            }

            LengthUnit::Mm => {
                self.length * <O as Orientation>::normalize(params.dpi_x, params.dpi_y)
                    / MM_PER_INCH
            }

            LengthUnit::Pt => {
                self.length * <O as Orientation>::normalize(params.dpi_x, params.dpi_y)
                    / POINTS_PER_INCH
            }

            LengthUnit::Pc => {
                self.length * <O as Orientation>::normalize(params.dpi_x, params.dpi_y)
                    / PICA_PER_INCH
            }
        }
    }
}

fn font_size_from_values(values: &ComputedValues, params: &ViewParams) -> f64 {
    let v = &values.font_size.0.value();

    match v.unit {
        LengthUnit::Percent => unreachable!("ComputedValues can't have a relative font size"),

        LengthUnit::Px => v.length,

        // This is the same default as used in Svg::get_size()
        LengthUnit::Em => v.length * 12.0,

        // This is the same default as used in Svg::get_size()
        LengthUnit::Ex => v.length * 12.0 / 2.0,

        // FontSize always is a Both, per properties.rs
        LengthUnit::In => v.length * Both::normalize(params.dpi_x, params.dpi_y),
        LengthUnit::Cm => v.length * Both::normalize(params.dpi_x, params.dpi_y) / CM_PER_INCH,
        LengthUnit::Mm => v.length * Both::normalize(params.dpi_x, params.dpi_y) / MM_PER_INCH,
        LengthUnit::Pt => v.length * Both::normalize(params.dpi_x, params.dpi_y) / POINTS_PER_INCH,
        LengthUnit::Pc => v.length * Both::normalize(params.dpi_x, params.dpi_y) / PICA_PER_INCH,
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
    Array(Vec<Length<Both>>),
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
fn parse_dash_array(parser: &mut Parser<'_, '_>) -> Result<Vec<Length<Both>>, ValueErrorKind> {
    let mut dasharray = Vec::new();

    loop {
        dasharray.push(Length::<Both>::parse(parser).and_then(Length::<Both>::check_nonnegative)?);

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
            Length::<Horizontal>::parse_str("42"),
            Ok(Length::<Horizontal>::new(42.0, LengthUnit::Px))
        );

        assert_eq!(
            Length::<Horizontal>::parse_str("-42px"),
            Ok(Length::<Horizontal>::new(-42.0, LengthUnit::Px))
        );
    }

    #[test]
    fn parses_percent() {
        assert_eq!(
            Length::<Horizontal>::parse_str("50.0%"),
            Ok(Length::<Horizontal>::new(0.5, LengthUnit::Percent))
        );
    }

    #[test]
    fn parses_font_em() {
        assert_eq!(
            Length::<Vertical>::parse_str("22.5em"),
            Ok(Length::<Vertical>::new(22.5, LengthUnit::Em))
        );
    }

    #[test]
    fn parses_font_ex() {
        assert_eq!(
            Length::<Vertical>::parse_str("22.5ex"),
            Ok(Length::<Vertical>::new(22.5, LengthUnit::Ex))
        );
    }

    #[test]
    fn parses_physical_units() {
        assert_eq!(
            Length::<Both>::parse_str("72pt"),
            Ok(Length::<Both>::new(72.0, LengthUnit::Pt))
        );

        assert_eq!(
            Length::<Both>::parse_str("-22.5in"),
            Ok(Length::<Both>::new(-22.5, LengthUnit::In))
        );

        assert_eq!(
            Length::<Both>::parse_str("-254cm"),
            Ok(Length::<Both>::new(-254.0, LengthUnit::Cm))
        );

        assert_eq!(
            Length::<Both>::parse_str("254mm"),
            Ok(Length::<Both>::new(254.0, LengthUnit::Mm))
        );

        assert_eq!(
            Length::<Both>::parse_str("60pc"),
            Ok(Length::<Both>::new(60.0, LengthUnit::Pc))
        );
    }

    #[test]
    fn empty_length_yields_error() {
        assert!(is_parse_error(&Length::<Both>::parse_str("")));
    }

    #[test]
    fn invalid_unit_yields_error() {
        assert!(is_parse_error(&Length::<Both>::parse_str("8furlong")));
    }

    #[test]
    fn check_nonnegative_works() {
        assert!(Length::<Both>::parse_str("0")
            .and_then(|l| l.check_nonnegative())
            .is_ok());
        assert!(Length::<Both>::parse_str("-10")
            .and_then(|l| l.check_nonnegative())
            .is_err());
    }

    #[test]
    fn normalize_default_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::<Both>::new(10.0, LengthUnit::Px).normalize(&values, &params),
            10.0
        );
    }

    #[test]
    fn normalize_absolute_units_works() {
        let params = ViewParams::new(40.0, 50.0, 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::<Horizontal>::new(10.0, LengthUnit::In).normalize(&values, &params),
            400.0
        );
        assert_approx_eq_cairo!(
            Length::<Vertical>::new(10.0, LengthUnit::In).normalize(&values, &params),
            500.0
        );

        assert_approx_eq_cairo!(
            Length::<Horizontal>::new(10.0, LengthUnit::Cm).normalize(&values, &params),
            400.0 / CM_PER_INCH
        );
        assert_approx_eq_cairo!(
            Length::<Horizontal>::new(10.0, LengthUnit::Mm).normalize(&values, &params),
            400.0 / MM_PER_INCH
        );
        assert_approx_eq_cairo!(
            Length::<Horizontal>::new(10.0, LengthUnit::Pt).normalize(&values, &params),
            400.0 / POINTS_PER_INCH
        );
        assert_approx_eq_cairo!(
            Length::<Horizontal>::new(10.0, LengthUnit::Pc).normalize(&values, &params),
            400.0 / PICA_PER_INCH
        );
    }

    #[test]
    fn normalize_percent_works() {
        let params = ViewParams::new(40.0, 40.0, 100.0, 200.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::<Horizontal>::new(0.05, LengthUnit::Percent).normalize(&values, &params),
            5.0
        );
        assert_approx_eq_cairo!(
            Length::<Vertical>::new(0.05, LengthUnit::Percent).normalize(&values, &params),
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
            Length::<Vertical>::new(1.0, LengthUnit::Em).normalize(&values, &params),
            12.0
        );

        assert_approx_eq_cairo!(
            Length::<Vertical>::new(1.0, LengthUnit::Ex).normalize(&values, &params),
            6.0
        );
    }

    fn parse_dash_array_str(s: &str) -> Result<Dasharray, ValueErrorKind> {
        Dasharray::parse_str(s)
    }

    #[test]
    fn parses_dash_array() {
        // helper to cut down boilderplate
        let length_parse = |s| Length::<Both>::parse_str(s).unwrap();

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
