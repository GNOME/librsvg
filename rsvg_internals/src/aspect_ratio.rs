//! Handling of `preserveAspectRatio` values
//!
//! This module handles `preserveAspectRatio` values [per the SVG specification][spec].
//! We have an [`AspectRatio`] struct which encapsulates such a value.
//!
//! ```
//! assert_eq!(AspectRatio::parse("xMidYMid", ()),
//!            Ok(AspectRatio { defer: false,
//!                             align: Align::Aligned { align: AlignMode::XmidYmid,
//!                                                     fit: FitMode::Meet, }, }));
//! ```
//!
//! [`AspectRatio`]: struct.AspectRatio.html
//! [spec]: https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute

use error::*;
use parsers::Parse;
use parsers::ParseError;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FitMode {
    Meet,
    Slice,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AlignMode {
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
pub enum Align {
    None,
    Aligned { align: AlignMode, fit: FitMode },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AspectRatio {
    pub defer: bool,
    pub align: Align,
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
    pub fn compute(&self,
                   object_width: f64,
                   object_height: f64,
                   dest_x: f64,
                   dest_y: f64,
                   dest_width: f64,
                   dest_height: f64)
                   -> (f64, f64, f64, f64) {
        match self.align {
            Align::None => (dest_x, dest_y, dest_width, dest_height),

            Align::Aligned { align, fit } => {
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
    fn default() -> Align {
        Align::Aligned { align: AlignMode::XmidYmid,
                         fit: FitMode::Meet, }
    }
}

impl Default for AspectRatio {
    fn default() -> AspectRatio {
        AspectRatio { defer: false,
                      align: Default::default(), }
    }
}

fn parse_align_mode(s: &str) -> Option<Align> {
    match s {
        "none" => Some(Align::None),
        "xMinYMin" => Some(Align::Aligned { align: AlignMode::XminYmin,
                                            fit: FitMode::Meet, }),
        "xMidYMin" => Some(Align::Aligned { align: AlignMode::XmidYmin,
                                            fit: FitMode::Meet, }),
        "xMaxYMin" => Some(Align::Aligned { align: AlignMode::XmaxYmin,
                                            fit: FitMode::Meet, }),
        "xMinYMid" => Some(Align::Aligned { align: AlignMode::XminYmid,
                                            fit: FitMode::Meet, }),
        "xMidYMid" => Some(Align::Aligned { align: AlignMode::XmidYmid,
                                            fit: FitMode::Meet, }),
        "xMaxYMid" => Some(Align::Aligned { align: AlignMode::XmaxYmid,
                                            fit: FitMode::Meet, }),
        "xMinYMax" => Some(Align::Aligned { align: AlignMode::XminYmax,
                                            fit: FitMode::Meet, }),
        "xMidYMax" => Some(Align::Aligned { align: AlignMode::XmidYmax,
                                            fit: FitMode::Meet, }),
        "xMaxYMax" => Some(Align::Aligned { align: AlignMode::XmaxYmax,
                                            fit: FitMode::Meet, }),
        _ => None,
    }
}

fn parse_fit_mode(s: &str) -> Option<FitMode> {
    match s {
        "meet" => Some(FitMode::Meet),
        "slice" => Some(FitMode::Slice),
        _ => None,
    }
}

enum ParseState {
    Defer,
    Align,
    Fit,
    Finished,
}

fn make_err() -> AttributeError {
    AttributeError::Parse(ParseError::new("expected \"[defer] <align> [meet | slice]\""))
}

impl Parse for AspectRatio {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: ()) -> Result<AspectRatio, AttributeError> {
        let mut defer = false;
        let mut align: Align = Default::default();
        let mut fit_mode = FitMode::Meet;

        let mut state = ParseState::Defer;

        for v in s.split_whitespace() {
            match state {
                ParseState::Defer => {
                    if v == "defer" {
                        defer = true;
                        state = ParseState::Align;
                    } else if let Some(parsed_align) = parse_align_mode(v) {
                        align = parsed_align;
                        state = ParseState::Fit;
                    } else {
                        return Err(make_err());
                    }
                }

                ParseState::Align => {
                    if let Some(parsed_align) = parse_align_mode(v) {
                        align = parsed_align;
                        state = ParseState::Fit;
                    } else {
                        return Err(make_err());
                    }
                }

                ParseState::Fit => {
                    if let Some(parsed_fit) = parse_fit_mode(v) {
                        fit_mode = parsed_fit;
                        state = ParseState::Finished;
                    } else {
                        return Err(make_err());
                    }
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

        Ok(AspectRatio { defer,
                         align: match align {
                             Align::None => Align::None,
                             Align::Aligned { align, .. } => Align::Aligned { align,
                                                                              fit: fit_mode, },
                         }, })
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
        assert_eq!(AspectRatio::parse("defer none", ()),
                   Ok(AspectRatio { defer: true,
                                    align: Align::None, }));

        assert_eq!(AspectRatio::parse("xMidYMid", ()),
                   Ok(AspectRatio { defer: false,
                                    align: Align::Aligned { align: AlignMode::XmidYmid,
                                                            fit: FitMode::Meet, }, }));

        assert_eq!(AspectRatio::parse("defer xMidYMid", ()),
                   Ok(AspectRatio { defer: true,
                                    align: Align::Aligned { align: AlignMode::XmidYmid,
                                                            fit: FitMode::Meet, }, }));

        assert_eq!(AspectRatio::parse("defer xMinYMax", ()),
                   Ok(AspectRatio { defer: true,
                                    align: Align::Aligned { align: AlignMode::XminYmax,
                                                            fit: FitMode::Meet, }, }));

        assert_eq!(AspectRatio::parse("defer xMaxYMid meet", ()),
                   Ok(AspectRatio { defer: true,
                                    align: Align::Aligned { align: AlignMode::XmaxYmid,
                                                            fit: FitMode::Meet, }, }));

        assert_eq!(AspectRatio::parse("defer xMinYMax slice", ()),
                   Ok(AspectRatio { defer: true,
                                    align: Align::Aligned { align: AlignMode::XminYmax,
                                                            fit: FitMode::Slice, }, }));
    }

    #[test]
    fn aligns() {
        assert_eq!(AspectRatio::parse("xMinYMin meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (0.0, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMinYMin slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, 0.0, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMinYMid meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (0.0, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMinYMid slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, -49.5, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMinYMax meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (0.0, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMinYMax slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, -99.0, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMidYMin meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (4.95, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMidYMin slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, 0.0, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMidYMid meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (4.95, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMidYMid slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, -49.5, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMidYMax meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (4.95, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMidYMax slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, -99.0, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMaxYMin meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (9.9, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMaxYMin slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, 0.0, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMaxYMid meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (9.9, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMaxYMid slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, -49.5, 10.0, 100.0));

        assert_eq!(AspectRatio::parse("xMaxYMax meet", ()).unwrap()
                                                          .compute(1.0, 10.0, 0.0, 0.0, 10.0, 1.0),
                   (9.9, 0.0, 0.1, 1.0));
        assert_eq!(AspectRatio::parse("xMaxYMax slice", ()).unwrap()
                                                           .compute(1.0,
                                                                    10.0,
                                                                    0.0,
                                                                    0.0,
                                                                    10.0,
                                                                    1.0),
                   (0.0, -99.0, 10.0, 100.0));
    }
}
