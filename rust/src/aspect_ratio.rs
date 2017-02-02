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

extern crate libc;
extern crate glib;

use self::glib::translate::*;

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
    pub defer: bool,
    pub align: Align
}

enum Align1D {
    Min,
    Mid,
    Max
}

fn align_1d (a: Align1D, dest_pos: f64, dest_size: f64, obj_size: f64) -> f64 {
    match a {
        Align1D::Min => { dest_pos },
        Align1D::Mid => { dest_pos + (dest_size - obj_size) / 2.0 },
        Align1D::Max => { dest_pos + dest_size - obj_size }
    }
}

impl AspectRatio {
    //! Returns (x, y, width, height)
    pub fn compute (&self,
                    object_width: f64,
                    object_height: f64,
                    dest_x: f64,
                    dest_y: f64,
                    dest_width: f64,
                    dest_height: f64) -> (f64, f64, f64, f64) {
        match self.align {
            Align::None => { (dest_x, dest_y, dest_width, dest_height) }

            Align::Aligned { align, fit } => {
                let w_factor = dest_width / object_width;
                let h_factor = dest_height / object_height;
                let factor: f64;

                match fit {
                    FitMode::Meet  => { factor = w_factor.min (h_factor); }
                    FitMode::Slice => { factor = w_factor.max (h_factor); }
                }

                let w = object_width * factor;
                let h = object_height * factor;

                let xalign: Align1D;
                let yalign: Align1D;

                match align {
                    AlignMode::XminYmin => { xalign = Align1D::Min; yalign = Align1D::Min; },
                    AlignMode::XminYmid => { xalign = Align1D::Min; yalign = Align1D::Mid; },
                    AlignMode::XminYmax => { xalign = Align1D::Min; yalign = Align1D::Max; },
                    AlignMode::XmidYmin => { xalign = Align1D::Mid; yalign = Align1D::Min; },
                    AlignMode::XmidYmid => { xalign = Align1D::Mid; yalign = Align1D::Mid; },
                    AlignMode::XmidYmax => { xalign = Align1D::Mid; yalign = Align1D::Max; },
                    AlignMode::XmaxYmin => { xalign = Align1D::Max; yalign = Align1D::Min; },
                    AlignMode::XmaxYmid => { xalign = Align1D::Max; yalign = Align1D::Mid; },
                    AlignMode::XmaxYmax => { xalign = Align1D::Max; yalign = Align1D::Max; }
                }

                let xpos = align_1d (xalign, dest_x, dest_width, w);
                let ypos = align_1d (yalign, dest_y, dest_height, h);

                (xpos, ypos, w, h)
            }
        }
    }
}

impl Default for Align {
    fn default () -> Align {
        Align::Aligned {
            align: AlignMode::XmidYmid,
            fit: FitMode::Meet
        }
    }
}

impl Default for AspectRatio {
    fn default () -> AspectRatio {
        AspectRatio {
            defer: false,
            align: Default::default ()
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

#[no_mangle]
pub extern fn rsvg_aspect_ratio_parse (c_str: *const libc::c_char) -> u32 {
    let my_str = unsafe { &String::from_glib_none (c_str) };
    let parsed = AspectRatio::from_str (my_str);

    match parsed {
        Ok (aspect_ratio) => { aspect_ratio_to_u32 (aspect_ratio) },
        Err (_) => {
            // We can't propagate the error here, so just return a default value
            let a: AspectRatio = Default::default ();
            aspect_ratio_to_u32 (a)
        }
    }
}

#[no_mangle]
pub extern fn rsvg_aspect_ratio_compute (aspect: u32,
                                         object_width: f64,
                                         object_height: f64,
                                         dest_x: *mut f64,
                                         dest_y: *mut f64,
                                         dest_width: *mut f64,
                                         dest_height: *mut f64) {
    unsafe {
        let (x, y, w, h) = u32_to_aspect_ratio (aspect).compute (object_width, object_height, *dest_x, *dest_y, *dest_width, *dest_height);
        *dest_x = x;
        *dest_y = y;
        *dest_width = w;
        *dest_height = h;
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

    #[test]
    fn aligns () {
        assert_eq! (AspectRatio::from_str ("XminYmin meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XminYmin slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XminYmid meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XminYmid slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, -49.5, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XminYmax meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XminYmax slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, -99.0, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XmidYmin meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (4.95, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XmidYmin slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XmidYmid meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (4.95, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XmidYmid slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, -49.5, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XmidYmax meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (4.95, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XmidYmax slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, -99.0, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XmaxYmin meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (9.9, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XmaxYmin slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, 0.0, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XmaxYmid meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (9.9, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XmaxYmid slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, -49.5, 10.0, 100.0));

        assert_eq! (AspectRatio::from_str ("XmaxYmax meet").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (9.9, 0.0, 0.1, 1.0));
        assert_eq! (AspectRatio::from_str ("XmaxYmax slice").unwrap().compute (1.0, 10.0, 0.0, 0.0, 10.0, 1.0), (0.0, -99.0, 10.0, 100.0));
    }
}
