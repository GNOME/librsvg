use glib::translate::*;
use itertools::Itertools;
use libc;
use util::utf8_cstr;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum XmlSpace {
    Default,
    Preserve,
}

/// Implements `xml:space` handling per the SVG spec
///
/// Normalizes a string as it comes out of the XML parser's handler
/// for character data according to the SVG rules in
/// <https://www.w3.org/TR/SVG/text.html#WhiteSpace>
pub fn xml_space_normalize(mode: XmlSpace, s: &str) -> String {
    match mode {
        XmlSpace::Default => normalize_default(s),
        XmlSpace::Preserve => normalize_preserve(s),
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
fn normalize_default(s: &str) -> String {
    s.trim().chars()
     .filter(|ch| *ch != '\n')
     .map(|ch| match ch {
              '\t' => ' ',
              c => c,
          })
     .coalesce(|current, next| match (current, next) {
                   (' ', ' ') => Ok(' '),
                   (_, _) => Err((current, next)),
               })
     .collect::<String>()
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
fn normalize_preserve(s: &str) -> String {
    s.chars().map(|ch| match ch {
              '\n' | '\t' => ' ',

              c => c,
          })
     .collect()
}

#[no_mangle]
pub extern "C" fn rsvg_xml_space_normalize(mode: XmlSpace,
                                           s: *const libc::c_char)
                                           -> *const libc::c_char {
    let rs = unsafe { utf8_cstr(s) };

    xml_space_normalize(mode, rs).to_glib_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_space_default() {
        assert_eq!(xml_space_normalize(XmlSpace::Default,
                                       "\n    WS example\n    indented lines\n  "),
                   "WS example indented lines");
        assert_eq!(xml_space_normalize(XmlSpace::Default,
                                       "\n  \t  \tWS \t\t\texample\n  \t  indented lines\t\t  \
                                        \n  "),
                   "WS example indented lines");
        assert_eq!(xml_space_normalize(XmlSpace::Default, "\n  \t  \tWS \t\t\texample\n  \t  duplicate letters\t\t  \n  "),
                   "WS example duplicate letters");
        assert_eq!(xml_space_normalize(XmlSpace::Default, "\nWS example\nnon-indented lines\n  "),
                   "WS examplenon-indented lines");
        assert_eq!(xml_space_normalize(XmlSpace::Default, "\nWS example\tnon-indented lines\n  "),
                   "WS example non-indented lines");
    }

    #[test]
    fn xml_space_preserve() {
        assert_eq!(xml_space_normalize(XmlSpace::Preserve,
                                       "\n    WS example\n    indented lines\n  "),
                   "     WS example     indented lines   ");
        assert_eq!(xml_space_normalize(XmlSpace::Preserve,
                                       "\n  \t  \tWS \t\t\texample\n  \t  indented lines\t\t  \
                                        \n  "),
                   "       WS    example      indented lines       ");
        assert_eq!(xml_space_normalize(XmlSpace::Preserve, "\n  \t  \tWS \t\t\texample\n  \t  duplicate letters\t\t  \n  "),
                   "       WS    example      duplicate letters       ");
    }
}
