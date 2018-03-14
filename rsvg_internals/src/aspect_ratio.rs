//! Handling of `preserveAspectRatio` values
//!
//! This module handles `preserveAspectRatio` values [per the SVG specification][spec].
//! We have an [`AspectRatio`] struct which encapsulates such a value.
//!
//! ```
//! assert_eq!(
//!     AspectRatio::parse("xMidYMid", ()),
//!     Ok(AspectRatio {
//!         defer: false,
//!         align: Some(Align {
//!             align: AlignMode::XmidYmid,
//!             fit: FitMode::Meet,
//!         }),
//!     })
//! );
//! ```
//!
//! [`AspectRatio`]: struct.AspectRatio.html
//! [spec]: https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

use error::*;
use parsers::Parse;
use parsers::ParseError;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AspectRatio {
    defer: bool,
    align: Option<Align>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FitMode {
    Meet,
    Slice,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum AlignMode {
    XminYmin,
    XmidYmin,
    XmaxYmin,
    XminYmid,
    XmidYmid,
    XmaxYmid,
    XminYmax,
    XmidYmax,
    XmaxYmax,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Align {
    align: AlignMode,
    fit: FitMode,
}

#[derive(Debug, Copy, Clone)]
enum Align1D {
    Min,
    Mid,
    Max,
}

fn align_1d(a: Align1D, dest_pos: f64, dest_size: f64, obj_size: f64) -> f64 {
    match a {
        Align1D::Min => dest_pos,
        Align1D::Mid => dest_pos + (dest_size - obj_size) / 2.0,
        Align1D::Max => dest_pos + dest_size - obj_size,
    }
}

impl AspectRatio {
    pub fn is_slice(&self) -> bool {
        match self.align {
            Some(Align {
                fit: FitMode::Slice,
                ..
            }) => true,

            _ => false
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

            Some(Align { align, fit }) => {
                let w_factor = dest_width / object_width;
                let h_factor = dest_height / object_height;
                let factor: f64;

                match fit {
                    FitMode::Meet => {
                        factor = w_factor.min(h_factor);
                    }
                    FitMode::Slice => {
                        factor = w_factor.max(h_factor);
                    }
                }

                let w = object_width * factor;
                let h = object_height * factor;

                let xalign: Align1D;
                let yalign: Align1D;

                match align {
                    AlignMode::XminYmin => {
                        xalign = Align1D::Min;
                        yalign = Align1D::Min;
                    }
                    AlignMode::XminYmid => {
                        xalign = Align1D::Min;
                        yalign = Align1D::Mid;
                    }
                    AlignMode::XminYmax => {
                        xalign = Align1D::Min;
                        yalign = Align1D::Max;
                    }
                    AlignMode::XmidYmin => {
                        xalign = Align1D::Mid;
                        yalign = Align1D::Min;
                    }
                    AlignMode::XmidYmid => {
                        xalign = Align1D::Mid;
                        yalign = Align1D::Mid;
                    }
                    AlignMode::XmidYmax => {
                        xalign = Align1D::Mid;
                        yalign = Align1D::Max;
                    }
                    AlignMode::XmaxYmin => {
                        xalign = Align1D::Max;
                        yalign = Align1D::Min;
                    }
                    AlignMode::XmaxYmid => {
                        xalign = Align1D::Max;
                        yalign = Align1D::Mid;
                    }
                    AlignMode::XmaxYmax => {
                        xalign = Align1D::Max;
                        yalign = Align1D::Max;
                    }
                }

                let xpos = align_1d(xalign, dest_x, dest_width, w);
                let ypos = align_1d(yalign, dest_y, dest_height, h);

                (xpos, ypos, w, h)
            }
        }
    }
}

impl Default for Align {
    fn default() -> Self {
        Align {
            align: AlignMode::XmidYmid,
            fit: FitMode::Meet,
        }
    }
}

impl Default for AspectRatio {
    fn default() -> AspectRatio {
        AspectRatio {
            defer: false,
            align: Some(Align::default()),
        }
    }
}

fn parse_align(s: &str) -> Result<Option<AlignMode>, AttributeError> {
    match s {
        "none" => Ok(None),

        "xMinYMin" => Ok(Some(AlignMode::XminYmin)),
        "xMidYMin" => Ok(Some(AlignMode::XmidYmin)),
        "xMaxYMin" => Ok(Some(AlignMode::XmaxYmin)),

        "xMinYMid" => Ok(Some(AlignMode::XminYmid)),
        "xMidYMid" => Ok(Some(AlignMode::XmidYmid)),
        "xMaxYMid" => Ok(Some(AlignMode::XmaxYmid)),

        "xMinYMax" => Ok(Some(AlignMode::XminYmax)),
        "xMidYMax" => Ok(Some(AlignMode::XmidYmax)),
        "xMaxYMax" => Ok(Some(AlignMode::XmaxYmax)),

        _ => Err(make_err()),
    }
}

fn parse_fit_mode(s: &str) -> Result<FitMode, AttributeError> {
    match s {
        "meet" => Ok(FitMode::Meet),
        "slice" => Ok(FitMode::Slice),
        _ => Err(make_err()),
    }
}

enum ParseState {
    Defer,
    Align,
    Fit,
    Finished,
}

fn make_err() -> AttributeError {
    AttributeError::Parse(ParseError::new(
        "expected \"[defer] <align> [meet | slice]\"",
    ))
}

impl Parse for AspectRatio {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: ()) -> Result<AspectRatio, AttributeError> {
        let mut defer = false;
        let mut align = Some(AlignMode::XmidYmid);
        let mut fit_mode = FitMode::Meet;

        let mut state = ParseState::Defer;

        for v in s.split_whitespace() {
            match state {
                ParseState::Defer => {
                    if v == "defer" {
                        defer = true;
                        state = ParseState::Align;
                    } else {
                        align = parse_align(v)?;
                        state = ParseState::Fit;
                    }
                },

                ParseState::Align => {
                    align = parse_align(v)?;
                    state = ParseState::Fit;
                },

                ParseState::Fit => {
                    fit_mode = parse_fit_mode(v)?;
                    state = ParseState::Finished;
                },

                _ => {
                    return Err(make_err());
                }
            }
        }

        // The string must match "[defer] <align> [meet | slice]".
        // Since the meet|slice is optional, we can end up in either
        // of the following states:
        match state {
            ParseState::Fit | ParseState::Finished => {}
            _ => {
                return Err(make_err());
            }
        }

        Ok(AspectRatio {
            defer,
            align: match align {
                None => None,
                Some(align_mode) => Some(Align {
                    align: align_mode,
                    fit: fit_mode,
                }),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_invalid_strings_yields_error() {
        assert!(AspectRatio::parse("", ()).is_err());

        assert!(AspectRatio::parse("defer", ()).is_err());

        assert!(AspectRatio::parse("defer foo", ()).is_err());

        assert!(AspectRatio::parse("defer xmidymid", ()).is_err());

        assert!(AspectRatio::parse("defer xMidYMid foo", ()).is_err());

        assert!(AspectRatio::parse("xmidymid", ()).is_err());

        assert!(AspectRatio::parse("xMidYMid foo", ()).is_err());

        assert!(AspectRatio::parse("defer xMidYMid meet foo", ()).is_err());
    }

    #[test]
    fn parses_valid_strings() {
        assert_eq!(
            AspectRatio::parse("defer none", ()),
            Ok(AspectRatio {
                defer: true,
                align: None,
            })
        );

        assert_eq!(
            AspectRatio::parse("xMidYMid", ()),
            Ok(AspectRatio {
                defer: false,
                align: Some(Align {
                    align: AlignMode::XmidYmid,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMidYMid", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    align: AlignMode::XmidYmid,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMinYMax", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    align: AlignMode::XminYmax,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMaxYMid meet", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    align: AlignMode::XmaxYmid,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMinYMax slice", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    align: AlignMode::XminYmax,
                    fit: FitMode::Slice,
                }),
            })
        );
    }

    #[test]
    fn aligns() {
        assert_eq!(
            AspectRatio::parse("xMinYMin meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMinYMin slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, 0.0, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMinYMid meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMinYMid slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, -49.5, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMinYMax meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMinYMax slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, -99.0, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMidYMin meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (4.95, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMidYMin slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, 0.0, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMidYMid meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (4.95, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMidYMid slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, -49.5, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMidYMax meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (4.95, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMidYMax slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, -99.0, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMaxYMin meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (9.9, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMaxYMin slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, 0.0, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMaxYMid meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (9.9, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMaxYMid slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, -49.5, 10.0, 100.0)
        );

        assert_eq!(
            AspectRatio::parse("xMaxYMax meet", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (9.9, 0.0, 0.1, 1.0)
        );
        assert_eq!(
            AspectRatio::parse("xMaxYMax slice", ()).unwrap().compute(
                1.0,
                10.0,
                0.0,
                0.0,
                10.0,
                1.0
            ),
            (0.0, -99.0, 10.0, 100.0)
        );
    }
}
