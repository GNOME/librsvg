use ::glib::translate::*;
use ::libc;

use std::f64::consts::*;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use parsers;
use parsers::ParseError;
use error::*;

/* Keep this in sync with ../../rsvg-private.h:LengthUnit */
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthUnit {
    Default,
    Percent,
    FontEm,
    FontEx,
    Inch,
    RelativeLarger,
    RelativeSmaller
}

/* Keep this in sync with ../../rsvg-private.h:LengthDir */
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LengthDir {
    Horizontal,
    Vertical,
    Both
}

/* This is *not* an opaque struct; it is actually visible to the C code.  It is so
 * that the remaining C code can create RsvgLength values as part of existing
 * structures or objects, without allocations on the heap.
 */
/* Keep this in sync with ../../rsvg-private.h:RsvgLength */
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct RsvgLength {
    pub length: f64,
    pub unit: LengthUnit,
    dir: LengthDir
}

impl Default for RsvgLength {
    fn default () -> RsvgLength {
        RsvgLength {
            length: 0.0,
            unit:   LengthUnit::Default,
            dir:    LengthDir::Both
        }
    }
}

const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH:     f64 = 2.54;
const MM_PER_INCH:     f64 = 25.4;
const PICA_PER_INCH:   f64 = 6.0;

fn compute_named_size (name: &str) -> f64 {
    let power: f64;

    match name {
        "xx-small" => { power = -3.0; },
        "x-small"  => { power = -2.0; },
        "small"    => { power = -1.0; },
        "medium"   => { power = 0.0; },
        "large"    => { power = 1.0; },
        "x-large"  => { power = 2.0; },
        "xx-large" => { power = 3.0; },
        _          => { unreachable! (); }
    }

    12.0 * 1.2f64.powf (power) / POINTS_PER_INCH
}

#[no_mangle]
pub extern fn rsvg_length_parse (string: *const libc::c_char, dir: LengthDir) -> RsvgLength {
    let my_string = unsafe { &String::from_glib_none (string) };

    // FIXME: this ignores errors; propagate them upstream
    RsvgLength::parse (my_string, dir).unwrap_or (RsvgLength::default ())
}

/* https://www.w3.org/TR/SVG/types.html#DataTypeLength
 * https://www.w3.org/TR/2008/REC-CSS2-20080411/syndata.html#length-units
 *
 * Lengths have units.  When they need to be need resolved to
 * units in the user's coordinate system, some unit types
 * need to know if they are horizontal/vertical/both.  For example,
 * a some_object.width="50%" is 50% with respect to the current
 * viewport's width.  In this case, the @dir argument is used
 * inside RsvgLength::normalize(), when it needs to know to what the
 * length refers.
 */

fn make_err () -> AttributeError {
    AttributeError::Parse (ParseError::new ("expected length: number(\"em\" | \"ex\" | \"px\" | \"in\" | \"cm\" | \"mm\" | \"pt\" | \"pc\" | \"%\")?"))
}

impl RsvgLength {
    pub fn parse (string: &str, dir: LengthDir) -> Result <RsvgLength, AttributeError> {
        let r = parsers::number_and_units (string);

        match r {
            Ok ((value, unit)) => {
                match unit {
                    "%" => Ok (RsvgLength { length: value * 0.01, // normalize to [0, 1]
                                            unit:   LengthUnit::Percent,
                                            dir:    dir }),

                    "em" => Ok (RsvgLength { length: value,
                                             unit:   LengthUnit::FontEm,
                                             dir:    dir }),

                    "ex" => Ok (RsvgLength { length: value,
                                             unit:   LengthUnit::FontEx,
                                             dir:    dir }),

                    "pt" => Ok (RsvgLength { length: value / POINTS_PER_INCH,
                                             unit:   LengthUnit::Inch,
                                             dir:    dir }),

                    "in" => Ok (RsvgLength { length: value,
                                             unit:   LengthUnit::Inch,
                                             dir:    dir }),

                    "cm" => Ok (RsvgLength { length: value / CM_PER_INCH,
                                             unit:   LengthUnit::Inch,
                                             dir:    dir }),

                    "mm" => Ok (RsvgLength { length: value / MM_PER_INCH,
                                             unit:   LengthUnit::Inch,
                                             dir:    dir }),

                    "pc" => Ok (RsvgLength { length: value / PICA_PER_INCH,
                                             unit:   LengthUnit::Inch,
                                             dir:    dir }),

                    "px" |
                    "" => Ok (RsvgLength { length: value,
                                           unit:   LengthUnit::Default,
                                           dir:    dir }),

                    _ => Err (make_err ())
                }
            },

            // FIXME: why are the following in Length?  They should be in FontSize
            _ => match string {
                "larger" => Ok (RsvgLength { length: 0.0,
                                             unit:   LengthUnit::RelativeLarger,
                                             dir:    dir }),

                "smaller" => Ok (RsvgLength { length: 0.0,
                                              unit:  LengthUnit::RelativeSmaller,
                                              dir:   dir }),

                "xx-small" |
                "x-small" |
                "small" |
                "medium" |
                "large" |
                "x-large" |
                "xx-large" => Ok (RsvgLength { length: compute_named_size (string),
                                               unit:   LengthUnit::Inch,
                                               dir:    dir }),

                _ => Err (make_err ())
            }
        }
    }

    pub fn normalize (&self, draw_ctx: *const RsvgDrawingCtx) -> f64 {
        match self.unit {
            LengthUnit::Default => {
                self.length
            },

            LengthUnit::Percent => {
                let (width, height) = drawing_ctx::get_view_box_size (draw_ctx);

                match self.dir {
                    LengthDir::Horizontal => { self.length * width },
                    LengthDir::Vertical   => { self.length * height },
                    LengthDir::Both       => { self.length * viewport_percentage (width, height) }
                }
            },

            LengthUnit::FontEm => {
                self.length * drawing_ctx::get_normalized_font_size (draw_ctx)
            },

            LengthUnit::FontEx => {
                self.length * drawing_ctx::get_normalized_font_size (draw_ctx) / 2.0
            },

            LengthUnit::Inch => {
                let (dpi_x, dpi_y) = drawing_ctx::get_dpi (draw_ctx);

                match self.dir {
                    LengthDir::Horizontal => { self.length * dpi_x },
                    LengthDir::Vertical   => { self.length * dpi_y },
                    LengthDir::Both       => { self.length * viewport_percentage (dpi_x, dpi_y) }
                }
            },

            // FIXME: these are pending: https://www.w3.org/TR/2008/REC-CSS2-20080411/fonts.html#propdef-font-size
            LengthUnit::RelativeLarger |
            LengthUnit::RelativeSmaller => { 0.0 }
        }
    }

    pub fn hand_normalize (&self,
                           pixels_per_inch: f64,
                           width_or_height: f64,
                           font_size: f64) -> f64 {
        match self.unit {
            LengthUnit::Default => { self.length },

            LengthUnit::Percent => { self.length * width_or_height },

            LengthUnit::FontEm => { self.length * font_size },

            LengthUnit::FontEx => { self.length * font_size / 2.0 },

            LengthUnit::Inch => { self.length * pixels_per_inch },

            _ => { 0.0 }
        }
    }
}

fn viewport_percentage (x: f64, y: f64) -> f64 {
    /* https://www.w3.org/TR/SVG/coords.html#Units
     *
     * "For any other length value expressed as a percentage of the viewport, the
     * percentage is calculated as the specified percentage of
     * sqrt((actual-width)**2 + (actual-height)**2))/sqrt(2)."
     */
    return (x * x + y * y).sqrt () / SQRT_2;
}

#[no_mangle]
pub extern fn rsvg_length_normalize (raw_length: *const RsvgLength, draw_ctx: *const RsvgDrawingCtx) -> f64 {
    assert! (!raw_length.is_null ());

    let length: &RsvgLength = unsafe { &*raw_length };

    length.normalize (draw_ctx)
}

#[no_mangle]
pub extern fn rsvg_length_hand_normalize (raw_length: *const RsvgLength,
                                          pixels_per_inch: f64,
                                          width_or_height: f64,
                                          font_size: f64) -> f64 {
    assert! (!raw_length.is_null ());

    let length: &RsvgLength = unsafe { &*raw_length };

    length.hand_normalize (pixels_per_inch, width_or_height, font_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default () {
        assert_eq! (RsvgLength::parse ("42", LengthDir::Horizontal),
                    Ok (RsvgLength {
                        length: 42.0,
                        unit:   LengthUnit::Default,
                        dir:    LengthDir::Horizontal}));

        assert_eq! (RsvgLength::parse ("-42px", LengthDir::Horizontal),
                    Ok (RsvgLength {
                        length: -42.0,
                        unit:   LengthUnit::Default,
                        dir:    LengthDir::Horizontal}));
    }

    #[test]
    fn parses_percent () {
        assert_eq! (RsvgLength::parse ("50.0%", LengthDir::Horizontal),
                    Ok (RsvgLength {
                        length: 0.5,
                        unit:   LengthUnit::Percent,
                        dir:    LengthDir::Horizontal}));
    }

    #[test]
    fn parses_font_em () {
        assert_eq! (RsvgLength::parse ("22.5em", LengthDir::Vertical),
                    Ok (RsvgLength {
                        length: 22.5,
                        unit:   LengthUnit::FontEm,
                        dir:    LengthDir::Vertical }));
    }

    #[test]
    fn parses_font_ex () {
        assert_eq! (RsvgLength::parse ("22.5ex", LengthDir::Vertical),
                    Ok (RsvgLength {
                        length: 22.5,
                        unit:   LengthUnit::FontEx,
                        dir:    LengthDir::Vertical }));
    }

    #[test]
    fn parses_physical_units () {
        assert_eq! (RsvgLength::parse ("72pt", LengthDir::Both),
                    Ok (RsvgLength {
                        length: 1.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both }));

        assert_eq! (RsvgLength::parse ("-22.5in", LengthDir::Both),
                    Ok (RsvgLength {
                        length: -22.5,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both }));

        assert_eq! (RsvgLength::parse ("-25.4cm", LengthDir::Both),
                    Ok (RsvgLength {
                        length: -10.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both }));

        assert_eq! (RsvgLength::parse ("254mm", LengthDir::Both),
                    Ok (RsvgLength {
                        length: 10.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both }));

        assert_eq! (RsvgLength::parse ("60pc", LengthDir::Both),
                    Ok (RsvgLength {
                        length: 10.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both }));
    }

    #[test]
    fn parses_relative_larger () {
        assert_eq! (RsvgLength::parse ("larger", LengthDir::Vertical),
                    Ok (RsvgLength {
                        length: 0.0,
                        unit:   LengthUnit::RelativeLarger,
                        dir:    LengthDir::Vertical }));
    }

    #[test]
    fn parses_relative_smaller () {
        assert_eq! (RsvgLength::parse ("smaller", LengthDir::Vertical),
                    Ok (RsvgLength {
                        length: 0.0,
                        unit:   LengthUnit::RelativeSmaller,
                        dir:    LengthDir::Vertical }));
    }

    #[test]
    fn parses_named_sizes () {
        let names = vec![ "xx-small",
                           "x-small",
                           "small",
                           "medium",
                           "large",
                           "x-large",
                           "xx-large" ];

        let mut previous_value: Option<f64> = None;

        // Just ensure that the values are progressively larger; don't
        // enforce a particular sequence.

        for name in names {
            let length = RsvgLength::parse (name, LengthDir::Both).unwrap ();

            assert_eq! (length.unit, LengthUnit::Inch);
            assert_eq! (length.dir, LengthDir::Both);

            if let Some (v) = previous_value {
                assert! (length.length > v);
            } else {
                previous_value = Some (length.length);
            }

        }
    }

    #[test]
    fn empty_length_yields_error () {
        assert! (is_parse_error (&RsvgLength::parse ("", LengthDir::Both)));
    }

    #[test]
    fn invalid_unit_yields_error () {
        assert! (is_parse_error (&RsvgLength::parse ("8furlong", LengthDir::Both)));
    }

    #[test]
    fn invalid_font_size_yields_error () {
        // FIXME: this is intended to test the (absence of) the "larger" et al values.
        // Since they really be in FontSize, not RsvgLength, we should remember
        // to move this test to that type later.
        assert! (is_parse_error (&RsvgLength::parse ("furlong", LengthDir::Both)));
    }
}
