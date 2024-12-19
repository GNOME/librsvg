// ! development file for text2

use crate::element::ElementTrait;
use crate::node::{Node, NodeBorrow};

#[allow(dead_code)]
#[derive(Default)]
pub struct Text2;

impl ElementTrait for Text2 {}

fn collect_text_from_node(node: &Node) -> String {
    // This function is basically the same as
    // text.rs::extract_chars_children_to_chunks_recursively()
    // You can do in the end:
    //
    //   if child.is_chars() {
    //       let contents = child.borrow_chars().get_string();
    //           ^^^^^^^^ append this to your result
    unimplemented!();
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
}
