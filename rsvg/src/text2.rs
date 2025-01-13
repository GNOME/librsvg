// ! development file for text2

use rctree::NodeEdge;

use crate::element::{Element, ElementData, ElementTrait};
use crate::node::{Node, NodeData};
use crate::text::BidiControl;

#[allow(dead_code)]
#[derive(Default)]
pub struct Text2;

impl ElementTrait for Text2 {}

#[derive(Default)]
struct Character {
    // https://www.w3.org/TR/SVG2/text.html#TextLayoutAlgorithm
    // Section "11.5.1 Setup"
    //
    // global_index: u32,
    // x: f64,
    // y: f64,
    // angle: Angle,
    // hidden: bool,

    addressable: bool,

    // middle: bool,
    // anchored_chunk: bool,
}

//              <tspan>   hello</tspan>
// addressable:        tffttttt

//              <tspan direction="ltr">A <tspan direction="rtl"> B </tspan> C</tspan>
//              A xx B xx C          "xx" are bidi control characters
// addressable: ttfffttffft

fn collapse_white_space(input: &str, white_space: WhiteSpace) -> Vec::<Character> {
    // HOMEWORK
    unimplemented!()
}


fn get_bidi_control(element: &Element) -> BidiControl {
    // Extract bidi control logic to separate function to avoid duplication
    let computed_values = element.get_computed_values();

    let unicode_bidi = computed_values.unicode_bidi();
    let direction = computed_values.direction();

    BidiControl::from_unicode_bidi_and_direction(unicode_bidi, direction)
}

// FIXME: Remove the following line when this code actually starts getting used outside of tests.
#[allow(unused)]
fn collect_text_from_node(node: &Node) -> String {
    let mut result = String::new();

    for edge in node.traverse() {
        match edge {
            NodeEdge::Start(child_node) => match *child_node.borrow() {
                NodeData::Text(ref text) => {
                    result.push_str(&text.get_string());
                }

                NodeData::Element(ref element) => match element.element_data {
                    ElementData::TSpan(_) | ElementData::Text(_) => {
                        let bidi_control = get_bidi_control(element);

                        for &ch in bidi_control.start {
                            result.push(ch);
                        }
                    }
                    _ => {}
                },
            },

            NodeEdge::End(child_node) => {
                if let NodeData::Element(ref element) = *child_node.borrow() {
                    match element.element_data {
                        ElementData::TSpan(_) | ElementData::Text(_) => {
                            let bidi_control = get_bidi_control(element);

                            for &ch in bidi_control.end {
                                result.push(ch);
                            }
                        }

                        _ => {}
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::element::ElementData;
    use crate::node::NodeBorrow;

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

    // Takes a string made of 't' and 'f' characters, and compares it
    // to the `addressable` field of the Characters slice.
    fn check_true_false_template(template: &str, characters: &[Character]) {
        // HOMEWORK
        // it's a loop with assert_eq!(characters[i].addressable, ...);
    }

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_normal_trivial_case() {
        let result = collapse_white_space("hello  world", WhiteSpace::Normal);
        let expected =                    "ttttttfttttt";
        check_true_false_template(expected, &result);
    }

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_normal_start_of_the_line() {
        let result = collapse_white_space("   hello  world", WhiteSpace::Normal);
        let expected =                    "tffttttttfttttt";
        check_true_false_template(expected, &result);
    }

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_normal_ignores_bidi_control() {
        let result = collapse_white_space("A \u{202b} B \u{202c} C", WhiteSpace::Normal);
        let expected =                    "ttffttfft";
        check_true_false_template(expected, &result);
    }
    
}
