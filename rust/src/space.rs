use libc;
use glib::translate::*;
use itertools::Itertools;
use std::borrow::Cow;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum XmlSpace {
    Default,
    Preserve
}

/// Implements `xml:space` handling per the SVG spec
///
/// Normalizes a string as it comes out of the XML parser's handler
/// for character data according to the SVG rules in
/// https://www.w3.org/TR/SVG/text.html#WhiteSpace
pub fn xml_space_normalize(mode: XmlSpace, s: &str) -> String {
    match mode {
        XmlSpace::Default => normalize_default(s),
        XmlSpace::Preserve => normalize_preserve(s)
    }
}

// From https://www.w3.org/TR/SVG/text.html#WhiteSpace
//
// When xml:space="default", the SVG user agent will do the following
// using a copy of the original character data content. First, it will
// remove all newline characters. Then it will convert all tab
// characters into space characters. Then, it will strip off all
// leading and trailing space characters. Then, all contiguous space
// characters will be consolidated.
fn normalize_default<'a, S: Into<Cow<'a, str>>>(s: S) -> String {
    let s = s.into();
    s.chars()
        .filter(|ch| *ch != '\n')
        .collect::<String>()
        // split at whitespace, also trims whitespace.
        .split_whitespace()
        .join(" ")
}

// From https://www.w3.org/TR/SVG/text.html#WhiteSpace
//
// When xml:space="preserve", the SVG user agent will do the following
// using a copy of the original character data content. It will
// convert all newline and tab characters into space characters. Then,
// it will draw all space characters, including leading, trailing and
// multiple contiguous space characters. Thus, when drawn with
// xml:space="preserve", the string "a   b" (three spaces between "a"
// and "b") will produce a larger separation between "a" and "b" than
// "a b" (one space between "a" and "b").
fn normalize_preserve<'a, S: Into<Cow<'a, str>>>(s: S) -> String {
    let s = s.into();
    let s = s.replace("\n", " ");
    s.replace("\t", " ")
}

#[no_mangle]
pub extern fn rsvg_xml_space_normalize(mode: XmlSpace, s: *const libc::c_char) -> *const libc::c_char {
    let rs = unsafe { String::from_glib_none (s) };

    xml_space_normalize(mode, &rs).to_glib_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_space_default() {
        assert_eq!(xml_space_normalize(XmlSpace::Default, "\n    WS example\n    indented lines\n  "),
                   "WS example indented lines");
        assert_eq!(xml_space_normalize(XmlSpace::Default, "\n  \t  \tWS \t\t\texample\n  \t  indented lines\t\t  \n  "),
                   "WS example indented lines");

        assert_eq!(xml_space_normalize(XmlSpace::Default, "\nWS example\nnon-indented lines\n  "),
                   "WS examplenon-indented lines");
    }

    #[test]
    fn xml_space_preserve() {
        assert_eq!(xml_space_normalize(XmlSpace::Preserve, "\n    WS example\n    indented lines\n  "),
                   "     WS example     indented lines   ");
        assert_eq!(xml_space_normalize(XmlSpace::Preserve, "\n  \t  \tWS \t\t\texample\n  \t  indented lines\t\t  \n  "),
                   "       WS    example      indented lines       ");

    }
}
