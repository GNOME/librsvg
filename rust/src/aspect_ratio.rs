//! Handling of preserveAspectRatio values
//!
//! This module handles preserveAspectRatio values [per the SVG specification][spec].
//! We have an [`AspectRatio`] struct which encapsulates such a value.
//!
//! [`AspectRatio`] implements `FromStr`, so it can be parsed easily:
//!
//! ```
//! assert_eq! (AspectRatio::from_str ("XmidYmid"),
//!             Ok (AspectRatio { defer: false,
//!                               align: Align::Aligned { align: AlignMode::XmidYmid,
//!                                                       fit: FitMode::Meet } }));
//! ```
//!
//! [`AspectRatio`]: struct.AspectRatio.html
//! [spec]: https://www.w3.org/TR/SVG/coords.html#PreserveAspectRatioAttribute


use std::fmt;
use std::str::FromStr;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FitMode {
    Meet,
    Slice
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
    XmaxYmax
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Align {
    None,
    Aligned {
        align: AlignMode,
        fit: FitMode
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AspectRatio {
    defer: bool,
    align: Align
}

impl Default for Align {
    fn default () -> Align {
        Align::Aligned {
            align: AlignMode::XmidYmid,
            fit: FitMode::Meet
        }
    }
}

bitflags! {
    flags AspectRatioFlags: u32 {
        const XMIN_YMIN = (1 << 0),
        const XMID_YMIN = (1 << 1),
        const XMAX_YMIN = (1 << 2),
        const XMIN_YMID = (1 << 3),
        const XMID_YMID = (1 << 4),
        const XMAX_YMID = (1 << 5),
        const XMIN_YMAX = (1 << 6),
        const XMID_YMAX = (1 << 7),
        const XMAX_YMAX = (1 << 8),
        const SLICE = (1 << 30),
        const DEFER = (1 << 31)
    }
}

pub fn aspect_ratio_to_u32 (a: AspectRatio) -> u32 {
    let mut val = AspectRatioFlags::empty ();

    if a.defer { val = val | DEFER; }

    match a.align {
        Align::None => { },

        Align::Aligned { align, fit } => {
            match align {
                AlignMode::XminYmin => { val = val | XMIN_YMIN; },
                AlignMode::XmidYmin => { val = val | XMID_YMIN; },
                AlignMode::XmaxYmin => { val = val | XMAX_YMIN; },
                AlignMode::XminYmid => { val = val | XMIN_YMID; },
                AlignMode::XmidYmid => { val = val | XMID_YMID; },
                AlignMode::XmaxYmid => { val = val | XMAX_YMID; },
                AlignMode::XminYmax => { val = val | XMIN_YMAX; },
                AlignMode::XmidYmax => { val = val | XMID_YMAX; },
                AlignMode::XmaxYmax => { val = val | XMAX_YMAX; },
            }

            match fit {
                FitMode::Meet  => { },
                FitMode::Slice => { val = val | SLICE; }
            }
        }
    }

    val.bits ()
}

pub fn u32_to_aspect_ratio (val: u32) -> AspectRatio {
    let val = AspectRatioFlags::from_bits (val).unwrap ();

    let defer = val.contains (DEFER);

    let mut aligned: bool = true;

    let align: AlignMode = {
        if val.contains (XMIN_YMIN)      { AlignMode::XminYmin }
        else if val.contains (XMID_YMIN) { AlignMode::XmidYmin }
        else if val.contains (XMAX_YMIN) { AlignMode::XmaxYmin }
        else if val.contains (XMIN_YMID) { AlignMode::XminYmid }
        else if val.contains (XMID_YMID) { AlignMode::XmidYmid }
        else if val.contains (XMAX_YMID) { AlignMode::XmaxYmid }
        else if val.contains (XMIN_YMAX) { AlignMode::XminYmax }
        else if val.contains (XMID_YMAX) { AlignMode::XmidYmax }
        else if val.contains (XMAX_YMAX) { AlignMode::XmaxYmax }
        else {
            aligned = false;
            AlignMode::XmidYmid
        }
    };

    let fit: FitMode = if val.contains(SLICE) { FitMode::Slice } else { FitMode::Meet };

    AspectRatio {
        defer: defer,
        align: if aligned {
            Align::Aligned {
                align: align,
                fit: fit
            }
        } else {
            Align::None
        }
    }
}

fn parse_align_mode (s: &str) -> Option<Align> {
    match s {
        "none"     => { Some (Align::None) },
        "XminYmin" => { Some (Align::Aligned { align: AlignMode::XminYmin, fit: FitMode::Meet } ) },
        "XmidYmin" => { Some (Align::Aligned { align: AlignMode::XmidYmin, fit: FitMode::Meet } ) },
        "XmaxYmin" => { Some (Align::Aligned { align: AlignMode::XmaxYmin, fit: FitMode::Meet } ) },
        "XminYmid" => { Some (Align::Aligned { align: AlignMode::XminYmid, fit: FitMode::Meet } ) },
        "XmidYmid" => { Some (Align::Aligned { align: AlignMode::XmidYmid, fit: FitMode::Meet } ) },
        "XmaxYmid" => { Some (Align::Aligned { align: AlignMode::XmaxYmid, fit: FitMode::Meet } ) },
        "XminYmax" => { Some (Align::Aligned { align: AlignMode::XminYmax, fit: FitMode::Meet } ) },
        "XmidYmax" => { Some (Align::Aligned { align: AlignMode::XmidYmax, fit: FitMode::Meet } ) },
        "XmaxYmax" => { Some (Align::Aligned { align: AlignMode::XmaxYmax, fit: FitMode::Meet } ) },
        _          => { None }
    }
}

fn parse_fit_mode (s: &str) -> Option<FitMode> {
    match s {
        "meet"  => { Some (FitMode::Meet) },
        "slice" => { Some (FitMode::Slice) },
        _       => { None }
    }
}

enum ParseState {
    Defer,
    Align,
    Fit,
    Finished
}

impl FromStr for AspectRatio {
    type Err = ParseAspectRatioError;

    fn from_str(s: &str) -> Result<AspectRatio, ParseAspectRatioError> {
        let mut defer = false;
        let mut align: Align = Default::default ();
        let mut fit_mode = FitMode::Meet;

        let mut state = ParseState::Defer;
        let mut iter = s.split_whitespace ();

        while let Some (v) = iter.next () {
            match state {
                ParseState::Defer => {
                    if v == "defer" {
                        defer = true;
                        state = ParseState::Align;
                    } else if let Some (parsed_align) = parse_align_mode (v) {
                        align = parsed_align;
                        state = ParseState::Fit;
                    } else {
                        return Err(ParseAspectRatioError);
                    }
                },

                ParseState::Align => {
                    if let Some (parsed_align) = parse_align_mode (v) {
                        align = parsed_align;
                        state = ParseState::Fit;
                    } else {
                        return Err(ParseAspectRatioError);
                    }
                },

                ParseState::Fit => {
                    if let Some (parsed_fit) = parse_fit_mode (v) {
                        fit_mode = parsed_fit;
                        state = ParseState::Finished;
                    } else {
                        return Err(ParseAspectRatioError);
                    }
                },

                _ => {
                    return Err(ParseAspectRatioError);
                }
            }
        }

        // The string must match "[defer] <align> [meet | slice]".
        // Since the meet|slice is optional, we can end up in either
        // of the following states:
        match state {
            ParseState::Fit | ParseState::Finished => {},
            _ => { return Err(ParseAspectRatioError); }
        }

        Ok (AspectRatio {
            defer: defer,
            align: match align {
                Align::None => { Align::None },
                Align::Aligned { align, ..} => {
                    Align::Aligned {
                        align: align,
                        fit: fit_mode
                    }
                }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseAspectRatioError;

impl fmt::Display for ParseAspectRatioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        "provided string did not match `[defer] <align> [meet | slice]`".fmt (f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parsing_invalid_strings_yields_error () {
        assert_eq! (AspectRatio::from_str (""), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("defer"), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("defer foo"), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("defer xmidymid"), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("defer XmidYmid foo"), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("xmidymid"), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("XmidYmid foo"), Err(ParseAspectRatioError));

        assert_eq! (AspectRatio::from_str ("defer XmidYmid meet foo"), Err(ParseAspectRatioError));
    }

    #[test]
    fn parses_valid_strings () {
        assert_eq! (AspectRatio::from_str ("XmidYmid"),
                    Ok (AspectRatio { defer: false,
                                      align: Align::Aligned { align: AlignMode::XmidYmid,
                                                              fit: FitMode::Meet } }));
        
        assert_eq! (AspectRatio::from_str ("defer XmidYmid"),
                    Ok (AspectRatio { defer: true,
                                      align: Align::Aligned { align: AlignMode::XmidYmid,
                                                              fit: FitMode::Meet } }));
        
        assert_eq! (AspectRatio::from_str ("defer XminYmax"),
                    Ok (AspectRatio { defer: true,
                                      align: Align::Aligned { align: AlignMode::XminYmax,
                                                              fit: FitMode::Meet } }));
        
        assert_eq! (AspectRatio::from_str ("defer XmaxYmid meet"),
                    Ok (AspectRatio { defer: true,
                                      align: Align::Aligned { align: AlignMode::XmaxYmid,
                                                              fit: FitMode::Meet } }));
        
        assert_eq! (AspectRatio::from_str ("defer XminYmax slice"),
                    Ok (AspectRatio { defer: true,
                                      align: Align::Aligned { align: AlignMode::XminYmax,
                                                              fit: FitMode::Slice } }));
    }

    fn test_roundtrip (s: &str) {
        let a = AspectRatio::from_str (s).unwrap ();

        assert_eq! (u32_to_aspect_ratio (aspect_ratio_to_u32 (a)), a);
    }

    #[test]
    fn conversion_to_u32_roundtrips () {
        test_roundtrip ("defer XmidYmid");
        test_roundtrip ("defer XminYmax slice");
        test_roundtrip ("XmaxYmax meet");
        test_roundtrip ("XminYmid slice");
    }
}
