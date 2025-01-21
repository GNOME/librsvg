// ! development file for text2

use rctree::NodeEdge;

use crate::element::{Element, ElementData, ElementTrait};
use crate::node::{Node, NodeData};
use crate::properties::WhiteSpace;
use crate::text::BidiControl;

#[allow(dead_code)]
#[derive(Default)]
pub struct Text2;

impl ElementTrait for Text2 {}

#[derive(Default)]
#[allow(dead_code)]
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
    character: char,
    // middle: bool,
    // anchored_chunk: bool,
}

//              <tspan>   hello</tspan>
// addressable:        tffttttt

//              <tspan direction="ltr">A <tspan direction="rtl"> B </tspan> C</tspan>
//              A xx B xx C          "xx" are bidi control characters
// addressable: ttfffttffft

// HOMEWORK
#[allow(unused)]
fn collapse_white_space(input: &str, white_space: WhiteSpace) -> Vec<Character> {
    match white_space {
        WhiteSpace::Normal => collapse_white_space_normal(input),
        _ => unimplemented!(),
    }
}

fn is_bidi_control(ch: char) -> bool {
    use crate::text::directional_formatting_characters::*;
    matches!(ch, LRE | RLE | LRO | RLO | PDF | LRI | RLI | FSI | PDI)
}

// move to inline constant if conditions needs to change
fn is_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n')
}

// Summary of white-space rules from https://www.w3.org/TR/css-text-3/#white-space-property
//
//              New Lines   Spaces and Tabs   Text Wrapping   End-of-line   End-of-line
//                                                            spaces        other space separators
// -----------------------------------------------------------------------------------------------
// normal       Collapse    Collapse          Wrap            Remove        Hang
// pre          Preserve    Preserve          No wrap         Preserve      No wrap
// nowrap       Collapse    Collapse          No wrap         Remove        Hang
// pre-wrap     Preserve    Preserve          Wrap            Hang          Hang
// break-spaces Preserve    Preserve          Wrap            Wrap          Wrap
// pre-line     Preserve    Collapse          Wrap            Remove        Hang


fn collapse_white_space_normal(input: &str) -> Vec<Character> {
    let mut result: Vec<Character> = Vec::with_capacity(input.len());
    let mut prev_was_space: bool = false;

    for ch in input.chars() {
        if is_bidi_control(ch) {
            result.push(Character {
                addressable: false,
                character: ch,
            });
            continue;
        }

        if is_space(ch) {
            if prev_was_space {
                result.push(Character {
                    addressable: false,
                    character: ch,
                });
            } else {
                result.push(Character {
                    addressable: true,
                    character: ch,
                });
                prev_was_space = true;
            }
        } else {
            result.push(Character {
                addressable: true,
                character: ch,
            });

            prev_was_space = false;
        }
    }

    result
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
        assert_eq!(characters.len(), template.len());

        // HOMEWORK
        // it's a loop with assert_eq!(characters[i].addressable, ...);
        for (i, ch) in template.chars().enumerate() {
            assert_eq!(characters[i].addressable, ch == 't');
        }
    }

    fn check_modes_with_identical_processing(
        string: &str,
        template: &str,
        mode1: WhiteSpace,
        mode2: WhiteSpace
    ) {
        let result1 = collapse_white_space(string, mode1);
        check_true_false_template(template, &result1);

        let result2 = collapse_white_space(string, mode2);
        check_true_false_template(template, &result2);
    }

    // white-space="normal" and "nowrap"; these are processed in the same way

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_normal_trivial_case() {
        check_modes_with_identical_processing(
            "hello  world",
            "ttttttfttttt",
            WhiteSpace::Normal,
            WhiteSpace::NoWrap
        );
    }

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_normal_start_of_the_line() {
        check_modes_with_identical_processing(
            "   hello  world",
            "tffttttttfttttt",
            WhiteSpace::Normal,
            WhiteSpace::NoWrap
        );
    }

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_normal_ignores_bidi_control() {
        check_modes_with_identical_processing(
            "A \u{202b} B \u{202c} C",
            "ttffttfft",
            WhiteSpace::Normal,
            WhiteSpace::NoWrap
        );
    }

    // white-space="pre" and "pre-wrap"; these are processed in the same way

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_pre_trivial_case() {
        check_modes_with_identical_processing(
            "   hello  \n  \n  \n\n\nworld",
            "tttttttttttttttttttttttt",
            WhiteSpace::Pre,
            WhiteSpace::PreWrap
        );
    }

    #[rustfmt::skip]
    #[test]
    fn handles_white_space_pre_ignores_bidi_control() {
        check_modes_with_identical_processing(
            "A  \u{202b} \n\n\n B \u{202c} C  ",
            "tttftttttttftttt",
            WhiteSpace::Pre,
            WhiteSpace::PreWrap
        );
    }
}
