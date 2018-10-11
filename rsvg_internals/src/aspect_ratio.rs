//! Handling of `preserveAspectRatio` values
//!
//! This module handles `preserveAspectRatio` values [per the SVG specification][spec].
//! We have an [`AspectRatio`] struct which encapsulates such a value.
//!
//! ```ignore
//! assert_eq!(
//!     AspectRatio::parse("xMidYMid", ()),
//!     Ok(AspectRatio {
//!         defer: false,
//!         align: Some(Align {
//!             x: X(Align1D::Mid),
//!             y: Y(Align1D::Mid),
//!             fit: FitMode::Meet,
//!         }),
//!     })
//! );
//! ```
//!
//! [`AspectRatio`]: struct.AspectRatio.html
//! [spec]: https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

use cssparser::{CowRcStr, Parser};
use error::ValueErrorKind;
use parsers::Parse;
use parsers::ParseError;
use std::ops::Deref;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AspectRatio {
    defer: bool,
    align: Option<Align>,
}

impl Default for AspectRatio {
    fn default() -> AspectRatio {
        AspectRatio {
            defer: false,
            align: Some(Align::default()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FitMode {
    Meet,
    Slice,
}

impl Default for FitMode {
    fn default() -> FitMode {
        FitMode::Meet
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Align {
    x: X,
    y: Y,
    fit: FitMode,
}

impl Default for Align {
    fn default() -> Align {
        Align {
            x: X(Align1D::Mid),
            y: Y(Align1D::Mid),
            fit: FitMode::default(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Align1D {
    Min,
    Mid,
    Max,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct X(Align1D);
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Y(Align1D);

impl Deref for X {
    type Target = Align1D;

    fn deref(&self) -> &Align1D {
        &self.0
    }
}

impl Deref for Y {
    type Target = Align1D;

    fn deref(&self) -> &Align1D {
        &self.0
    }
}

impl Align1D {
    fn compute(self, dest_pos: f64, dest_size: f64, obj_size: f64) -> f64 {
        match self {
            Align1D::Min => dest_pos,
            Align1D::Mid => dest_pos + (dest_size - obj_size) / 2.0,
            Align1D::Max => dest_pos + dest_size - obj_size,
        }
    }
}

impl AspectRatio {
    pub fn is_slice(&self) -> bool {
        match self.align {
            Some(Align {
                fit: FitMode::Slice,
                ..
            }) => true,

            _ => false,
        }
    }

    pub fn compute(
        &self,
        object_width: f64,
        object_height: f64,
        dest_x: f64,
        dest_y: f64,
        dest_width: f64,
        dest_height: f64,
    ) -> (f64, f64, f64, f64) {
        match self.align {
            None => (dest_x, dest_y, dest_width, dest_height),

            Some(Align { x, y, fit }) => {
                let w_factor = dest_width / object_width;
                let h_factor = dest_height / object_height;
                let factor = match fit {
                    FitMode::Meet => w_factor.min(h_factor),
                    FitMode::Slice => w_factor.max(h_factor),
                };

                let w = object_width * factor;
                let h = object_height * factor;

                let xpos = x.compute(dest_x, dest_width, w);
                let ypos = y.compute(dest_y, dest_height, h);

                (xpos, ypos, w, h)
            }
        }
    }
}

fn parse_align_xy(ident: &CowRcStr) -> Result<Option<(X, Y)>, ValueErrorKind> {
    use self::Align1D::*;

    match ident.as_ref() {
        "none" => Ok(None),

        "xMinYMin" => Ok(Some((X(Min), Y(Min)))),
        "xMidYMin" => Ok(Some((X(Mid), Y(Min)))),
        "xMaxYMin" => Ok(Some((X(Max), Y(Min)))),

        "xMinYMid" => Ok(Some((X(Min), Y(Mid)))),
        "xMidYMid" => Ok(Some((X(Mid), Y(Mid)))),
        "xMaxYMid" => Ok(Some((X(Max), Y(Mid)))),

        "xMinYMax" => Ok(Some((X(Min), Y(Max)))),
        "xMidYMax" => Ok(Some((X(Mid), Y(Max)))),
        "xMaxYMax" => Ok(Some((X(Max), Y(Max)))),

        _ => Err(ValueErrorKind::Parse(ParseError::new("invalid alignment"))),
    }
}

fn parse_fit_mode(s: &str) -> Result<FitMode, ValueErrorKind> {
    match s {
        "meet" => Ok(FitMode::Meet),
        "slice" => Ok(FitMode::Slice),
        _ => Err(ValueErrorKind::Parse(ParseError::new("invalid fit mode"))),
    }
}

impl Parse for AspectRatio {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: ()) -> Result<AspectRatio, ValueErrorKind> {
        let defer = parser.try(|p| p.expect_ident_matching("defer")).is_ok();

        let align_xy = parser.try(|p| {
            p.expect_ident()
                .map_err(|_| ValueErrorKind::Parse(ParseError::new("expected identifier")))
                .and_then(|ident| parse_align_xy(ident))
        })?;

        let fit = parser
            .try(|p| {
                p.expect_ident()
                    .map_err(|_| ValueErrorKind::Parse(ParseError::new("expected identifier")))
                    .and_then(|ident| parse_fit_mode(ident))
            })
            .unwrap_or(FitMode::default());

        parser
            .expect_exhausted()
            .map_err(|_| ValueErrorKind::Parse(ParseError::new("extra data in AspectRatio")))?;

        let align = align_xy.map(|(x, y)| Align { x, y, fit });

        Ok(AspectRatio { defer, align })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_eq_cairo::ApproxEqCairo;

    #[test]
    fn parsing_invalid_strings_yields_error() {
        assert!(AspectRatio::parse_str("", ()).is_err());
        assert!(AspectRatio::parse_str("defer", ()).is_err());
        assert!(AspectRatio::parse_str("defer foo", ()).is_err());
        assert!(AspectRatio::parse_str("defer xmidymid", ()).is_err());
        assert!(AspectRatio::parse_str("defer xMidYMid foo", ()).is_err());
        assert!(AspectRatio::parse_str("xmidymid", ()).is_err());
        assert!(AspectRatio::parse_str("xMidYMid foo", ()).is_err());
        assert!(AspectRatio::parse_str("defer xMidYMid meet foo", ()).is_err());
    }

    #[test]
    fn parses_valid_strings() {
        assert_eq!(
            AspectRatio::parse_str("defer none", ()),
            Ok(AspectRatio {
                defer: true,
                align: None,
            },)
        );

        assert_eq!(
            AspectRatio::parse_str("xMidYMid", ()),
            Ok(AspectRatio {
                defer: false,
                align: Some(Align {
                    x: X(Align1D::Mid),
                    y: Y(Align1D::Mid),
                    fit: FitMode::Meet,
                },),
            },)
        );

        assert_eq!(
            AspectRatio::parse_str("defer xMidYMid", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: X(Align1D::Mid),
                    y: Y(Align1D::Mid),
                    fit: FitMode::Meet,
                },),
            },)
        );

        assert_eq!(
            AspectRatio::parse_str("defer xMinYMax", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: X(Align1D::Min),
                    y: Y(Align1D::Max),
                    fit: FitMode::Meet,
                },),
            },)
        );

        assert_eq!(
            AspectRatio::parse_str("defer xMaxYMid meet", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: X(Align1D::Max),
                    y: Y(Align1D::Mid),
                    fit: FitMode::Meet,
                },),
            },)
        );

        assert_eq!(
            AspectRatio::parse_str("defer xMinYMax slice", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: X(Align1D::Min),
                    y: Y(Align1D::Max),
                    fit: FitMode::Slice,
                },),
            },)
        );
    }

    fn assert_quadruples_equal(a: &(f64, f64, f64, f64), b: &(f64, f64, f64, f64)) {
        assert_approx_eq_cairo!(a.0, b.0);
        assert_approx_eq_cairo!(a.1, b.1);
        assert_approx_eq_cairo!(a.2, b.2);
        assert_approx_eq_cairo!(a.3, b.3);
    }

    #[test]
    fn aligns() {
        let foo = AspectRatio::parse_str("xMinYMin meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMinYMin slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, 0.0, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMinYMid meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMinYMid slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, -49.5, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMinYMax meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMinYMax slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, -99.0, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMidYMin meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(4.95, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMidYMin slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, 0.0, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMidYMid meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(4.95, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMidYMid slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, -49.5, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMidYMax meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(4.95, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMidYMax slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, -99.0, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMaxYMin meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(9.9, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMaxYMin slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, 0.0, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMaxYMid meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(9.9, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMaxYMid slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, -49.5, 10.0, 100.0));

        let foo = AspectRatio::parse_str("xMaxYMax meet", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(9.9, 0.0, 0.1, 1.0));

        let foo = AspectRatio::parse_str("xMaxYMax slice", ()).unwrap();
        let foo = foo.compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0);
        assert_quadruples_equal(&foo, &(0.0, -99.0, 10.0, 100.0));
    }
}
