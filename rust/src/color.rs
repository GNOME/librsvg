use std::str::FromStr;

use parsers::ParseError;
use error::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RawColor {
    pub argb: u32
}

impl RawColor {
    pub fn new_rgb (r: u8, g: u8, b: u8) -> RawColor {
        RawColor {
            argb: (0xff000000       |
                   (r as u32) << 16 |
                   (g as u32) << 8  |
                   (b as u32))
        }
    }

    pub fn new_argb (a: u8, r: u8, g: u8, b: u8) -> RawColor {
        RawColor {
            argb: ((a as u32) << 24 |
                   (r as u32) << 16 |
                   (g as u32) << 8  |
                   (b as u32))
        }
    }
}

impl FromStr for RawColor {
    type Err = AttributeError;

    fn from_str (s: &str) -> Result<RawColor, AttributeError> {
        if s.starts_with ('#') {
            return parse_hex (s[1..].as_bytes ()).or (Err (
                AttributeError::Parse (ParseError::new ("expected one of #rrggbbaa, #rrggbb, #rgba, #rgb"))))
        }

        Err (AttributeError::Parse (ParseError::new ("invalid color specification")))
    }
}

fn hex_digit (c: u8) -> Result<u8, ()> {
    match c {
        b'0'...b'9' => Ok (c - b'0'),
        b'A'...b'F' => Ok (c - b'A' + 10),
        b'a'...b'f' => Ok (c - b'a' + 10),
        _ => Err (())
    }
}

fn parse_hex (s: &[u8]) -> Result<RawColor, ()> {
    match s.len () {
        8 => {
            // #rrggbbaa -> 0xaarrggbb
            Ok (RawColor::new_argb (hex_digit (s[6])? * 16 + hex_digit (s[7])?,
                                    hex_digit (s[0])? * 16 + hex_digit (s[1])?,
                                    hex_digit (s[2])? * 16 + hex_digit (s[3])?,
                                    hex_digit (s[4])? * 16 + hex_digit (s[5])?))
        },

        6 => {
            // #rrggbb -> 0xffrrggbb
            Ok (RawColor::new_rgb (hex_digit (s[0])? * 16 + hex_digit (s[1])?,
                                   hex_digit (s[2])? * 16 + hex_digit (s[3])?,
                                   hex_digit (s[4])? * 16 + hex_digit (s[5])?))
        },

        4 => {
            // #rgba -> 0xaarrggbb
            Ok (RawColor::new_argb (hex_digit (s[3])? * 0x11,
                                    hex_digit (s[0])? * 0x11,
                                    hex_digit (s[1])? * 0x11,
                                    hex_digit (s[2])? * 0x11))
        },

        3 => {
            // #rgb -> 0xffrrggbb
            Ok (RawColor::new_rgb (hex_digit (s[0])? * 0x11,
                                   hex_digit (s[1])? * 0x11,
                                   hex_digit (s[2])? * 0x11))
        }

        _ => Err (())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_hash_hex_colors () {
        assert_eq! (RawColor::from_str ("#AB10fa20").unwrap (), RawColor::new_argb (0x20, 0xab, 0x10, 0xfa));
        assert_eq! (RawColor::from_str ("#10fa20").unwrap (),   RawColor::new_rgb  (0x10, 0xfa, 0x20));
        assert_eq! (RawColor::from_str ("#abcd").unwrap (),     RawColor::new_argb (0xdd, 0xaa, 0xbb, 0xcc));
        assert_eq! (RawColor::from_str ("#123").unwrap (),      RawColor::new_rgb  (0x11, 0x22, 0x33));
    }

    #[test]
    fn invalid_hash_hex_colors_yield_error () {
        assert! (RawColor::from_str ("#").is_err ());
        assert! (RawColor::from_str ("#xyz").is_err ());
        assert! (RawColor::from_str ("#112233gg").is_err ());
    }

    #[test]
    fn invalid_colors_yield_error () {
        assert! (RawColor::from_str ("").is_err ());
        assert! (RawColor::from_str ("foo").is_err ());
    }
}
