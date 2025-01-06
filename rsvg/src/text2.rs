// ! development file for text2

use crate::element::{ElementData, ElementTrait};
use crate::node::{Node, NodeBorrow};
use crate::text::{wrap_with_direction_control_chars, BidiControl};

#[allow(dead_code)]
#[derive(Default)]
pub struct Text2;

impl ElementTrait for Text2 {}

// FIXME: Remove the following line when this code actually starts getting used outside of tests.
#[allow(unused)]
fn collect_text_from_node(node: &Node) -> String {
    let mut result = String::new();

    for child in node.children() {
        if child.is_chars() {
            let content = child.borrow_chars().get_string();
            result.push_str(&content);
        } else if child.is_element() {
            let element = child.borrow_element();
            let element_data = child.borrow_element_data();

            if matches!(*element_data, ElementData::TSpan(_)) {
                let values = element.get_computed_values();
                let direction = values.direction();
                let unicode_bidi = values.unicode_bidi();

                let bidi_control =
                    BidiControl::from_unicode_bidi_and_direction(unicode_bidi, direction);

                let original_string = collect_text_from_node(&child);

                let string_for_this_element =
                    wrap_with_direction_control_chars(&original_string, &bidi_control);

                result.push_str(&string_for_this_element);
            }
        } else {
            result.push_str(&collect_text_from_node(&child));
        };
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::element::ElementData;

    use super::*;

    #[test]
    fn collects_text_in_a_single_string() {
        let doc_str = br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="100" height="100">

  <text2 id="sample">
    Hello
    <tspan font-style="italic">
      <tspan font-weight="bold">bold</tspan>
      world!
    </tspan>
    How are you.
  </text2>
</svg>
"##;

        let document = Document::load_from_bytes(doc_str);

        let text2_node = document.lookup_internal_node("sample").unwrap();
        assert!(matches!(
            *text2_node.borrow_element_data(),
            ElementData::Text2(_)
        ));

        let text_string = collect_text_from_node(&text2_node);
        assert_eq!(
            text_string,
            "\n    \
             Hello\n    \
             \n      \
             bold\n      \
             world!\n    \
             \n    \
             How are you.\
             \n  "
        );
    }

    #[test]
    fn adds_bidi_control_characters() {
        let doc_str = br##"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="100" height="100">

  <text2 id="sample">
    Hello
    <tspan direction="rtl" unicode-bidi="embed">
      <tspan direction="ltr" unicode-bidi="isolate-override">bold</tspan>
      world!
    </tspan>
    How are <tspan direction="rtl" unicode-bidi="isolate">you</tspan>.
  </text2>
</svg>
"##;

        let document = Document::load_from_bytes(doc_str);

        let text2_node = document.lookup_internal_node("sample").unwrap();
        assert!(matches!(
            *text2_node.borrow_element_data(),
            ElementData::Text2(_)
        ));

        let text_string = collect_text_from_node(&text2_node);
        assert_eq!(
            text_string,
            "\n    \
             Hello\n    \
             \u{202b}\n      \
             \u{2068}\u{202d}bold\u{202c}\u{2069}\n      \
             world!\n    \
             \u{202c}\n    \
             How are \u{2067}you\u{2069}.\
             \n  "
        );
    }
}
