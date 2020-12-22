//! CSS length values.
//!
//! [`CssLength`] is the struct librsvg uses to represent CSS lengths.
//! See its documentation for examples of how to construct it.
//!
//! `CssLength` values need to know whether they will be normalized with respect to the width,
//! height, or both dimensions of the current viewport.  `CssLength` values can be signed or
//! unsigned.  So, a `CssLength` has two type parameters, [`Normalize`] and [`Validate`];
//! the full type is `CssLength<N: Normalize, V: Validate>`.  We provide [`Horizontal`],
//! [`Vertical`], and [`Both`] implementations of [`Normalize`]; these let length values know
//! how to normalize themselves with respect to the current viewport.  We also provide
//! [`Signed`] and [`Unsigned`] implementations of [`Validate`].
//!
//! For ease of use, we define two type aliases [`Length`] and [`ULength`] corresponding to
//! signed and unsigned.
//!
//! For example, the implementation of [`Circle`] defines this structure with fields for the
//! `(center_x, center_y, radius)`:
//!
//! ```
//! # use librsvg::doctest_only::{Length,ULength,Horizontal,Vertical,Both};
//! pub struct Circle {
//!     cx: Length<Horizontal>,
//!     cy: Length<Vertical>,
//!     r: ULength<Both>,
//! }
//! ```
//!
//! This means that:
//!
//! * `cx` and `cy` define the center of the circle, they can be positive or negative, and
//! they will be normalized with respect to the current viewport's width and height,
//! respectively.  If the SVG document specified `<circle cx="50%" cy="30%">`, the values
//! would be normalized to be at 50% of the the viewport's width, and 30% of the viewport's
//! height.
//!
//! * `r` is non-negative and needs to be resolved against the [normalized diagonal][diag]
//! of the current viewport.
//!
//! The `N` type parameter of `CssLength<N, I>` is enough to know how to normalize a length
//! value; the [`normalize`] method will handle it automatically.
//!
//! [`Circle`]: ../shapes/struct.Circle.html
//! [`CssLength`]: struct.CssLength.html
//! [`Length`]: type.Length.html
//! [`ULength`]: type.ULength.html
//! [`Horizontal`]: struct.Horizontal.html
//! [`Vertical`]: struct.Vertical.html
//! [`Both`]: struct.Both.html
//! [`Normalize`]: trait.Normalize.html
//! [`Signed`]: struct.Signed.html
//! [`Unsigned`]: struct.Unsigned.html
//! [`Validate`]: trait.Validate.html
//! [diag]: https://www.w3.org/TR/SVG/coords.html#Units
//! [`normalize`]: struct.CssLength.html#method.normalize

use cssparser::{Parser, Token};
use std::f64::consts::*;
use std::marker::PhantomData;

use crate::dpi::Dpi;
use crate::drawing_ctx::ViewParams;
use crate::error::*;
use crate::parsers::{finite_f32, Parse};
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
/// [CSS lengths]: https://www.w3.org/TR/CSS22/syndata.html#length-units
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

/// Used for the `N` type parameter of `CssLength<N: Normalize, V: Validate>`.
pub trait Normalize {
    /// Computes an orientation-based scaling factor.
    ///
    /// This is used in the [`CssLength.normalize`] method to resolve lengths with percentage
    /// units; they need to be resolved with respect to the width, height, or [normalized
    /// diagonal][diag] of the current viewport.
    ///
    /// [`CssLength.normalize`]: struct.CssLength.html#method.normalize
    /// [diag]: https://www.w3.org/TR/SVG/coords.html#Units
    fn normalize(x: f64, y: f64) -> f64;
}

/// Allows declaring `CssLength<Horizontal>`.
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Horizontal;

/// Allows declaring `CssLength<Vertical>`.
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Vertical;

/// Allows declaring `CssLength<Both>`.
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Both;

impl Normalize for Horizontal {
    #[inline]
    fn normalize(x: f64, _y: f64) -> f64 {
        x
    }
}

impl Normalize for Vertical {
    #[inline]
    fn normalize(_x: f64, y: f64) -> f64 {
        y
    }
}

impl Normalize for Both {
    #[inline]
    fn normalize(x: f64, y: f64) -> f64 {
        viewport_percentage(x, y)
    }
}

/// Used for the `V` type parameter of `CssLength<N: Normalize, V: Validate>`.
pub trait Validate {
    /// Checks if the specified value is acceptable
    ///
    /// This is used when parsing a length value
    fn validate(v: f64) -> Result<f64, ValueErrorKind> {
        Ok(v)
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Signed;

impl Validate for Signed {}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Unsigned;

impl Validate for Unsigned {
    fn validate(v: f64) -> Result<f64, ValueErrorKind> {
        if v >= 0.0 {
            Ok(v)
        } else {
            Err(ValueErrorKind::Value(
                "value must be non-negative".to_string(),
            ))
        }
    }
}

/// A CSS length value.
///
/// This is equivalent to [CSS lengths].
///
/// [CSS lengths]: https://www.w3.org/TR/CSS22/syndata.html#length-units
///
/// `CssLength` implements the [`Parse`] trait, so it can be parsed out of a
/// [`cssparser::Parser`].
///
/// This type will be normally used through the type aliases [`Length`] and [`ULength`]
///
/// Examples of construction:
///
/// ```
/// # use librsvg::doctest_only::{Length,ULength,LengthUnit,Horizontal,Vertical,Both};
/// # use librsvg::doctest_only::Parse;
/// // Explicit type
/// let width: Length<Horizontal> = Length::new(42.0, LengthUnit::Cm);
///
/// // Inferred type
/// let height = Length::<Vertical>::new(42.0, LengthUnit::Cm);
///
/// // Parsed
/// let radius = ULength::<Both>::parse_str("5px").unwrap();
/// ```
///
/// During the rendering phase, a `CssLength` needs to be normalized into the current
/// coordinate system's units with the [`normalize`] method.
///
/// [`Length`]: type.Length.html
/// [`ULength`]: type.ULength.html
/// [`Normalize`]: trait.Normalize.html
/// [`Horizontal`]: struct.Horizontal.html
/// [`Vertical`]: struct.Vertical.html
/// [`Both`]: struct.Both.html
/// [`new`]: #method.new
/// [`normalize`]: #method.normalize
/// [`cssparser::Parser`]: https://docs.rs/cssparser/0.27.1/cssparser/struct.Parser.html
/// [`Parse`]: ../parsers/trait.Parse.html
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct CssLength<N: Normalize, V: Validate> {
    /// Numeric part of the length
    pub length: f64,

    /// Unit part of the length
    pub unit: LengthUnit,

    /// Dummy; used internally for the type parameter `N`
    orientation: PhantomData<N>,

    /// Dummy; used internally for the type parameter `V`
    validation: PhantomData<V>,
}

impl<N: Normalize, V: Validate> From<CssLength<N, V>> for RsvgLength {
    fn from(l: CssLength<N, V>) -> RsvgLength {
        RsvgLength {
            length: l.length,
            unit: l.unit,
        }
    }
}

impl<N: Normalize, V: Validate> Default for CssLength<N, V> {
    fn default() -> Self {
        CssLength::new(0.0, LengthUnit::Px)
    }
}

pub const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH: f64 = 2.54;
const MM_PER_INCH: f64 = 25.4;
const PICA_PER_INCH: f64 = 6.0;

impl<N: Normalize, V: Validate> Parse for CssLength<N, V> {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<CssLength<N, V>, ParseError<'i>> {
        let l_value;
        let l_unit;

        let token = parser.next()?.clone();

        match token {
            Token::Number { value, .. } => {
                l_value = value;
                l_unit = LengthUnit::Px;
            }

            Token::Percentage { unit_value, .. } => {
                l_value = unit_value;
                l_unit = LengthUnit::Percent;
            }

            Token::Dimension {
                value, ref unit, ..
            } => {
                l_value = value;

                l_unit = match unit.as_ref() {
                    "px" => LengthUnit::Px,
                    "em" => LengthUnit::Em,
                    "ex" => LengthUnit::Ex,
                    "in" => LengthUnit::In,
                    "cm" => LengthUnit::Cm,
                    "mm" => LengthUnit::Mm,
                    "pt" => LengthUnit::Pt,
                    "pc" => LengthUnit::Pc,

                    _ => return Err(parser.new_unexpected_token_error(token)),
                };
            }

            _ => return Err(parser.new_unexpected_token_error(token)),
        }

        let l_value = f64::from(finite_f32(l_value).map_err(|e| parser.new_custom_error(e))?);

        <V as Validate>::validate(l_value)
            .map_err(|e| parser.new_custom_error(e))
            .map(|l_value| CssLength::new(l_value, l_unit))
    }
}

impl<N: Normalize, V: Validate> CssLength<N, V> {
    /// Creates a CssLength.
    ///
    /// The compiler needs to know the type parameters `N` and `V` which represents the
    /// length's orientation and validation.
    /// You can specify them explicitly, or call the parametrized method:
    ///
    /// ```
    /// # use librsvg::doctest_only::{Length,LengthUnit,Horizontal,Vertical};
    /// // Explicit type
    /// let width: Length<Horizontal> = Length::new(42.0, LengthUnit::Cm);
    ///
    /// // Inferred type
    /// let height = Length::<Vertical>::new(42.0, LengthUnit::Cm);
    /// ```
    pub fn new(l: f64, unit: LengthUnit) -> CssLength<N, V> {
        CssLength {
            length: l,
            unit,
            orientation: PhantomData,
            validation: PhantomData,
        }
    }

    /// Normalizes a specified length into a used value.
    ///
    /// Lengths may come with non-pixel units, and when rendering, they need to be normalized
    /// to pixels based on the current viewport (e.g. for lengths with percent units), and
    /// based on the current element's set of `ComputedValues` (e.g. for lengths with `Em`
    /// units that need to be resolved against the current font size).
    pub fn normalize(&self, values: &ComputedValues, params: &ViewParams) -> f64 {
        match self.unit {
            LengthUnit::Px => self.length,

            LengthUnit::Percent => {
                self.length * <N as Normalize>::normalize(params.vbox.width(), params.vbox.height())
            }

            LengthUnit::Em => self.length * font_size_from_values(values, params.dpi),

            LengthUnit::Ex => self.length * font_size_from_values(values, params.dpi) / 2.0,

            LengthUnit::In => self.length * <N as Normalize>::normalize(params.dpi.x, params.dpi.y),

            LengthUnit::Cm => {
                self.length * <N as Normalize>::normalize(params.dpi.x, params.dpi.y) / CM_PER_INCH
            }

            LengthUnit::Mm => {
                self.length * <N as Normalize>::normalize(params.dpi.x, params.dpi.y) / MM_PER_INCH
            }

            LengthUnit::Pt => {
                self.length * <N as Normalize>::normalize(params.dpi.x, params.dpi.y)
                    / POINTS_PER_INCH
            }

            LengthUnit::Pc => {
                self.length * <N as Normalize>::normalize(params.dpi.x, params.dpi.y)
                    / PICA_PER_INCH
            }
        }
    }
}

fn font_size_from_values(values: &ComputedValues, dpi: Dpi) -> f64 {
    let v = &values.font_size().value();

    match v.unit {
        LengthUnit::Percent => unreachable!("ComputedValues can't have a relative font size"),

        LengthUnit::Px => v.length,

        // The following implies that our default font size is 12, which
        // matches the default from the FontSize property.
        LengthUnit::Em => v.length * 12.0,
        LengthUnit::Ex => v.length * 12.0 / 2.0,

        // FontSize always is a Both, per properties.rs
        LengthUnit::In => v.length * Both::normalize(dpi.x, dpi.y),
        LengthUnit::Cm => v.length * Both::normalize(dpi.x, dpi.y) / CM_PER_INCH,
        LengthUnit::Mm => v.length * Both::normalize(dpi.x, dpi.y) / MM_PER_INCH,
        LengthUnit::Pt => v.length * Both::normalize(dpi.x, dpi.y) / POINTS_PER_INCH,
        LengthUnit::Pc => v.length * Both::normalize(dpi.x, dpi.y) / PICA_PER_INCH,
    }
}

fn viewport_percentage(x: f64, y: f64) -> f64 {
    // https://www.w3.org/TR/SVG/coords.html#Units
    // "For any other length value expressed as a percentage of the viewport, the
    // percentage is calculated as the specified percentage of
    // sqrt((actual-width)**2 + (actual-height)**2))/sqrt(2)."
    (x * x + y * y).sqrt() / SQRT_2
}

/// Alias for `CssLength` types that can have negative values
pub type Length<N> = CssLength<N, Signed>;

/// Alias for `CssLength` types that are non negative
pub type ULength<N> = CssLength<N, Unsigned>;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::float_eq_cairo::ApproxEqCairo;

    #[test]
    fn parses_default() {
        assert_eq!(
            Length::<Horizontal>::parse_str("42").unwrap(),
            Length::<Horizontal>::new(42.0, LengthUnit::Px)
        );

        assert_eq!(
            Length::<Horizontal>::parse_str("-42px").unwrap(),
            Length::<Horizontal>::new(-42.0, LengthUnit::Px)
        );
    }

    #[test]
    fn parses_percent() {
        assert_eq!(
            Length::<Horizontal>::parse_str("50.0%").unwrap(),
            Length::<Horizontal>::new(0.5, LengthUnit::Percent)
        );
    }

    #[test]
    fn parses_font_em() {
        assert_eq!(
            Length::<Vertical>::parse_str("22.5em").unwrap(),
            Length::<Vertical>::new(22.5, LengthUnit::Em)
        );
    }

    #[test]
    fn parses_font_ex() {
        assert_eq!(
            Length::<Vertical>::parse_str("22.5ex").unwrap(),
            Length::<Vertical>::new(22.5, LengthUnit::Ex)
        );
    }

    #[test]
    fn parses_physical_units() {
        assert_eq!(
            Length::<Both>::parse_str("72pt").unwrap(),
            Length::<Both>::new(72.0, LengthUnit::Pt)
        );

        assert_eq!(
            Length::<Both>::parse_str("-22.5in").unwrap(),
            Length::<Both>::new(-22.5, LengthUnit::In)
        );

        assert_eq!(
            Length::<Both>::parse_str("-254cm").unwrap(),
            Length::<Both>::new(-254.0, LengthUnit::Cm)
        );

        assert_eq!(
            Length::<Both>::parse_str("254mm").unwrap(),
            Length::<Both>::new(254.0, LengthUnit::Mm)
        );

        assert_eq!(
            Length::<Both>::parse_str("60pc").unwrap(),
            Length::<Both>::new(60.0, LengthUnit::Pc)
        );
    }

    #[test]
    fn parses_unsigned() {
        assert_eq!(
            ULength::<Horizontal>::parse_str("42").unwrap(),
            ULength::<Horizontal>::new(42.0, LengthUnit::Px)
        );

        assert_eq!(
            ULength::<Both>::parse_str("0pt").unwrap(),
            ULength::<Both>::new(0.0, LengthUnit::Pt)
        );

        assert!(ULength::<Horizontal>::parse_str("-42px").is_err());
    }

    #[test]
    fn empty_length_yields_error() {
        assert!(Length::<Both>::parse_str("").is_err());
    }

    #[test]
    fn invalid_unit_yields_error() {
        assert!(Length::<Both>::parse_str("8furlong").is_err());
    }

    #[test]
    fn normalize_default_works() {
        let params = ViewParams::new(Dpi::new(40.0, 40.0), 100.0, 100.0);

        let values = ComputedValues::default();

        assert_approx_eq_cairo!(
            Length::<Both>::new(10.0, LengthUnit::Px).normalize(&values, &params),
            10.0
        );
    }

    #[test]
    fn normalize_absolute_units_works() {
        let params = ViewParams::new(Dpi::new(40.0, 50.0), 100.0, 100.0);

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
        let params = ViewParams::new(Dpi::new(40.0, 40.0), 100.0, 200.0);

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
        let params = ViewParams::new(Dpi::new(40.0, 40.0), 100.0, 200.0);

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
}
