use std::f64::consts::*;

use cssparser::{Parser, Token};

use error::ValueErrorKind;
use parsers::{Parse, ParseError};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Angle(f64);

impl Angle {
    pub fn new(rad: f64) -> Angle {
        Angle(Angle::normalize(rad))
    }

    pub fn from_degrees(deg: f64) -> Angle {
        Angle(Angle::normalize(deg.to_radians()))
    }

    pub fn from_vector(vx: f64, vy: f64) -> Angle {
        let rad = vy.atan2(vx);

        if rad.is_nan() {
            Angle(0.0)
        } else {
            Angle(Angle::normalize(rad))
        }
    }

    pub fn radians(&self) -> f64 {
        self.0
    }

    pub fn bisect(&self, other: Angle) -> Angle {
        let half_delta = (other.0 - self.0) * 0.5;

        if FRAC_PI_2 < half_delta.abs() {
            Angle(Angle::normalize(self.0 + half_delta - PI))
        } else {
            Angle(Angle::normalize(self.0 + half_delta))
        }
    }

    // Normalizes an angle to [0.0, 2*PI)
    fn normalize(rad: f64) -> f64 {
        let res = rad % (PI * 2.0);
        if res < 0.0 {
            res + PI * 2.0
        } else {
            res
        }
    }
}

// angle:
// https://www.w3.org/TR/SVG/types.html#DataTypeAngle
//
// angle ::= number ("deg" | "grad" | "rad")?
//
impl Parse for Angle {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: ()) -> Result<Angle, ValueErrorKind> {
        let angle = {
            let token = parser
                .next()
                .map_err(|_| ParseError::new("expected angle"))?;

            match *token {
                Token::Number { value, .. } => Angle::from_degrees(value as f64),

                Token::Dimension {
                    value, ref unit, ..
                } => {
                    let value = f64::from(value);

                    match unit.as_ref() {
                        "deg" => Angle::from_degrees(value),
                        "grad" => Angle::from_degrees(value * 360.0 / 400.0),
                        "rad" => Angle::new(value),
                        _ => {
                            return Err(ValueErrorKind::Parse(ParseError::new(
                                "expected 'deg' | 'grad' | 'rad'",
                            )));
                        }
                    }
                }

                _ => return Err(ValueErrorKind::Parse(ParseError::new("expected angle"))),
            }
        };

        parser
            .expect_exhausted()
            .map_err(|_| ValueErrorKind::Parse(ParseError::new("expected angle")))?;

        Ok(angle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_angle() {
        assert_eq!(Angle::parse_str("0", ()), Ok(Angle::new(0.0)));
        assert_eq!(Angle::parse_str("15", ()), Ok(Angle::from_degrees(15.0)));
        assert_eq!(
            Angle::parse_str("180.5deg", ()),
            Ok(Angle::from_degrees(180.5))
        );
        assert_eq!(Angle::parse_str("1rad", ()), Ok(Angle::new(1.0)));
        assert_eq!(
            Angle::parse_str("-400grad", ()),
            Ok(Angle::from_degrees(-360.0))
        );

        assert!(Angle::parse_str("", ()).is_err());
        assert!(Angle::parse_str("foo", ()).is_err());
        assert!(Angle::parse_str("300foo", ()).is_err());
    }
}
