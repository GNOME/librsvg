extern crate libc;
extern crate glib;

use self::glib::translate::*;

use strtod::*;

/* Keep this in sync with ../../rsvg-private.h:LengthUnit */
#[repr(C)]
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
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
#[derive(Debug, PartialEq)]
pub struct RsvgLength {
    length: f64,
    unit: LengthUnit,
    dir: LengthDir
}

pub enum RsvgDrawingCtx {}

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
        _          => { return 0.0; }
    }

    12.0 * 1.2f64.powf (power) / POINTS_PER_INCH
}

#[no_mangle]
pub extern fn rsvg_length_parse (string: *const libc::c_char, dir: LengthDir) -> RsvgLength {
    let my_string = unsafe { &String::from_glib_none (string) };

    parse_length (my_string, dir)
}

/* https://www.w3.org/TR/SVG/types.html#DataTypeLength
 * https://www.w3.org/TR/2008/REC-CSS2-20080411/syndata.html#length-units
 *
 * Lengths have units.  When they need to be need resolved to
 * units in the user's coordinate system, some unit types
 * need to know if they are horizontal/vertical/both.  For example,
 * a some_object.width="50%" is 50% with respect to the current
 * viewport's width.  In this case, the @dir argument is used
 * when _rsvg_css_normalize_length() needs to know to what the
 * length refers.
 */
pub fn parse_length (string: &str, dir: LengthDir) -> RsvgLength {
    let unit: LengthUnit;

    let (mut value, rest) = strtod (string);

    match rest.as_ref () {
        "%" => {
            value *= 0.01; // normalize to [0, 1]
            unit = LengthUnit::Percent;
        },

        "em" => {
            unit = LengthUnit::FontEm;
        },

        "ex" => {
            unit = LengthUnit::FontEx;
        },

        "pt" => {
            value /= POINTS_PER_INCH;
            unit = LengthUnit::Inch;
        },

        "in" => {
            unit = LengthUnit::Inch;
        },

        "cm" => {
            value /= CM_PER_INCH;
            unit = LengthUnit::Inch;
        },

        "mm" => {
            value /= MM_PER_INCH;
            unit = LengthUnit::Inch;
        },

        "pc" => {
            value /= PICA_PER_INCH;
            unit = LengthUnit::Inch;
        },

        "larger" => {
            unit = LengthUnit::RelativeLarger;
        },

        "smaller" => {
            unit = LengthUnit::RelativeSmaller;
        },

        "xx-small" |
        "x-small" |
        "small" |
        "medium" |
        "large" |
        "x-large" |
        "xx-large" => {
            value = compute_named_size (rest);
            unit = LengthUnit::Inch;
        },

        _ => {
            unit = LengthUnit::Default;
        }
    }

    RsvgLength {
        length: value,
        unit: unit,
        dir: dir
    }
}

#[no_mangle]
pub extern fn rsvg_length_normalize (length: *const RsvgLength, draw_ctx: *const RsvgDrawingCtx) -> f64 {
    unimplemented! ();
}

#[no_mangle]
pub extern fn rsvg_length_hand_normalize (length: *const RsvgLength,
                                          pixels_per_inch: f64,
                                          width_or_height: f64,
                                          font_size: f64) -> f64 {
    unimplemented! ();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default () {
        assert_eq! (parse_length ("42", LengthDir::Horizontal),
                    RsvgLength {
                        length: 42.0,
                        unit:   LengthUnit::Default,
                        dir:    LengthDir::Horizontal});

        assert_eq! (parse_length ("-42px", LengthDir::Horizontal),
                    RsvgLength {
                        length: -42.0,
                        unit:   LengthUnit::Default,
                        dir:    LengthDir::Horizontal});
    }

    #[test]
    fn parses_percent () {
        assert_eq! (parse_length ("50.0%", LengthDir::Horizontal),
                    RsvgLength {
                        length: 0.5,
                        unit:   LengthUnit::Percent,
                        dir:    LengthDir::Horizontal});
    }

    #[test]
    fn parses_font_em () {
        assert_eq! (parse_length ("22.5em", LengthDir::Vertical),
                    RsvgLength {
                        length: 22.5,
                        unit:   LengthUnit::FontEm,
                        dir:    LengthDir::Vertical });
    }

    #[test]
    fn parses_font_ex () {
        assert_eq! (parse_length ("22.5ex", LengthDir::Vertical),
                    RsvgLength {
                        length: 22.5,
                        unit:   LengthUnit::FontEx,
                        dir:    LengthDir::Vertical });
    }

    #[test]
    fn parses_physical_units () {
        assert_eq! (parse_length ("72pt", LengthDir::Both),
                    RsvgLength {
                        length: 1.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both });

        assert_eq! (parse_length ("-22.5in", LengthDir::Both),
                    RsvgLength {
                        length: -22.5,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both });

        assert_eq! (parse_length ("-25.4cm", LengthDir::Both),
                    RsvgLength {
                        length: -10.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both });

        assert_eq! (parse_length ("254mm", LengthDir::Both),
                    RsvgLength {
                        length: 10.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both });

        assert_eq! (parse_length ("60pc", LengthDir::Both),
                    RsvgLength {
                        length: 10.0,
                        unit:   LengthUnit::Inch,
                        dir:    LengthDir::Both });
    }

    #[test]
    fn parses_relative_larger () {
        assert_eq! (parse_length ("larger", LengthDir::Vertical),
                    RsvgLength {
                        length: 0.0,
                        unit:   LengthUnit::RelativeLarger,
                        dir:    LengthDir::Vertical });
    }

    #[test]
    fn parses_relative_smaller () {
        assert_eq! (parse_length ("smaller", LengthDir::Vertical),
                    RsvgLength {
                        length: 0.0,
                        unit:   LengthUnit::RelativeSmaller,
                        dir:    LengthDir::Vertical });
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
            let length = parse_length (name, LengthDir::Both);

            assert_eq! (length.unit, LengthUnit::Inch);
            assert_eq! (length.dir, LengthDir::Both);

            if let Some (v) = previous_value {
                assert! (length.length > v);
            } else {
                previous_value = Some (length.length);
            }

        }

    }
}
