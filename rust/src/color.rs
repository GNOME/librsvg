use std::str::FromStr;

use parsers::ParseError;
use error::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RawColor {
    pub argb: u32
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
    let result: Result<u32, ()> = match s.len () {
        8 => {
            // #rrggbbaa -> 0xaarrggbb
            Ok (((hex_digit (s[0])? * 16 + hex_digit (s[1])?) as u32) << 16 |
                ((hex_digit (s[2])? * 16 + hex_digit (s[3])?) as u32) << 8  |
                ((hex_digit (s[4])? * 16 + hex_digit (s[5])?) as u32)       |
                ((hex_digit (s[6])? * 16 + hex_digit (s[7])?) as u32) << 24)
        },

        6 => {
            // #rrggbb -> 0xffrrggbb
            Ok (0xff000000 |
                ((hex_digit (s[0])? * 16 + hex_digit (s[1])?) as u32) << 16 |
                ((hex_digit (s[2])? * 16 + hex_digit (s[3])?) as u32) << 8  |
                ((hex_digit (s[4])? * 16 + hex_digit (s[5])?) as u32))
        },

        4 => {
            // #rgba -> 0xaarrggbb
            Ok (((hex_digit (s[0])? * 0x11) as u32) << 16 |
                ((hex_digit (s[1])? * 0x11) as u32) << 8  |
                ((hex_digit (s[2])? * 0x11) as u32)       |
                ((hex_digit (s[3])? * 0x11) as u32) << 24)  
        },

        3 => {
            // #rgb -> 0xffrrggbb
            Ok (0xff000000 |
                ((hex_digit (s[0])? * 0x11) as u32) << 16 |
                ((hex_digit (s[1])? * 0x11) as u32) << 8  |
                ((hex_digit (s[2])? * 0x11) as u32))
        }

        _ => Err (())
    };

    match result {
        Ok (argb) => Ok (RawColor { argb: argb }),
        Err (_)   => Err (())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_hash_hex_colors () {
        assert_eq! (RawColor::from_str ("#AB10fa20").unwrap (), RawColor { argb: 0x20ab10fa });
        assert_eq! (RawColor::from_str ("#10fa20").unwrap (),   RawColor { argb: 0xff10fa20 });
        assert_eq! (RawColor::from_str ("#abcd").unwrap (),     RawColor { argb: 0xddaabbcc });
        assert_eq! (RawColor::from_str ("#123").unwrap (),      RawColor { argb: 0xff112233 });
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
