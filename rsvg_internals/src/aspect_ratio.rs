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
//!             x: Align1D::Mid,
//!             y: Align1D::Mid,
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
struct Align {
    x: Align1D,
    y: Align1D,
    fit: FitMode,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Align1D {
    Min,
    Mid,
    Max,
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
            Some(Align { fit: FitMode::Slice, .. }) => true,

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

impl Default for AspectRatio {
    fn default() -> AspectRatio {
        AspectRatio {
            defer: false,
            align: Some(Align::default()),
        }
    }
}

impl Default for FitMode {
    fn default() -> FitMode {
        FitMode::Meet
    }
}

impl Default for Align {
    fn default() -> Align {
        Align {
            x: Align1D::Mid,
            y: Align1D::Mid,
            fit: FitMode::default(),
        }
    }
}

impl Align {
    fn parse(s: &str) -> Result<Option<Align>, AttributeError> {
        match s {
            "none" => Ok(None),

            "xMinYMin" => Ok(Some((Align1D::Min, Align1D::Min))),
            "xMidYMin" => Ok(Some((Align1D::Mid, Align1D::Min))),
            "xMaxYMin" => Ok(Some((Align1D::Max, Align1D::Min))),

            "xMinYMid" => Ok(Some((Align1D::Min, Align1D::Mid))),
            "xMidYMid" => Ok(Some((Align1D::Mid, Align1D::Mid))),
            "xMaxYMid" => Ok(Some((Align1D::Max, Align1D::Mid))),

            "xMinYMax" => Ok(Some((Align1D::Min, Align1D::Max))),
            "xMidYMax" => Ok(Some((Align1D::Mid, Align1D::Max))),
            "xMaxYMax" => Ok(Some((Align1D::Max, Align1D::Max))),

            _ => Err(make_err()),
        }.map(|xy| {
            xy.map(|(x, y)| {
                Align {
                    x,
                    y,
                    fit: FitMode::default(),
                }
            })
        })
    }
}

impl FitMode {
    fn parse(s: &str) -> Result<FitMode, AttributeError> {
        match s {
            "meet" => Ok(FitMode::Meet),
            "slice" => Ok(FitMode::Slice),
            _ => Err(make_err()),
        }
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
        let mut align = Some(Align::default());
        let mut fit_mode = FitMode::Meet;

        let mut state = ParseState::Defer;

        for v in s.split_whitespace() {
            match state {
                ParseState::Defer => {
                    if v == "defer" {
                        defer = true;
                        state = ParseState::Align;
                    } else {
                        align = Align::parse(v)?;
                        state = ParseState::Fit;
                    }
                }

                ParseState::Align => {
                    align = Align::parse(v)?;
                    state = ParseState::Fit;
                }

                ParseState::Fit => {
                    fit_mode = FitMode::parse(v)?;
                    state = ParseState::Finished;
                }

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
                Some(Align { x, y, .. }) => Some(Align {
                    x,
                    y,
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
                    x: Align1D::Mid,
                    y: Align1D::Mid,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMidYMid", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: Align1D::Mid,
                    y: Align1D::Mid,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMinYMax", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: Align1D::Min,
                    y: Align1D::Max,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMaxYMid meet", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: Align1D::Max,
                    y: Align1D::Mid,
                    fit: FitMode::Meet,
                }),
            })
        );

        assert_eq!(
            AspectRatio::parse("defer xMinYMax slice", ()),
            Ok(AspectRatio {
                defer: true,
                align: Some(Align {
                    x: Align1D::Min,
                    y: Align1D::Max,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
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
                1.0,
            ),
            (0.0, -99.0, 10.0, 100.0)
        );
    }
}
