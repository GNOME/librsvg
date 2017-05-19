use std::str::FromStr;

use parsers::ParseError;
use error::*;

use ::cssparser;

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
        match cssparser::Color::parse (&mut cssparser::Parser::new (s)) {
            Ok (cssparser::Color::RGBA (rgba)) => Ok (RawColor::new_argb (rgba.alpha, rgba.red, rgba.green, rgba.blue)),
            _ => Err (AttributeError::Parse (ParseError::new ("invalid color specification")))
        }
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
    fn parses_color_keywords () {
        assert_eq! (RawColor::from_str ("red").unwrap (),  RawColor::new_rgb (0xff, 0x00, 0x00));
        assert_eq! (RawColor::from_str ("lime").unwrap (), RawColor::new_rgb (0x00, 0xff, 0x00));
        assert_eq! (RawColor::from_str ("blue").unwrap (), RawColor::new_rgb (0x00, 0x00, 0xff));
    }

    #[test]
    fn parses_color_functions () {
        assert_eq! (RawColor::from_str ("rgb(255, 0, 0)").unwrap (), RawColor::new_rgb (0xff, 0x00, 0x00));
        assert_eq! (RawColor::from_str ("rgb(0, 255, 0)").unwrap (), RawColor::new_rgb (0x00, 0xff, 0x00));
        assert_eq! (RawColor::from_str ("rgb(0, 0, 255)").unwrap (), RawColor::new_rgb (0x00, 0x00, 0xff));
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
