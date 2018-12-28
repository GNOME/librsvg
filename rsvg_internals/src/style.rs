use attributes::Attribute;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use property_bag::PropertyBag;
use text::NodeChars;

use std::cell::RefCell;

/// Represents a <style> node.
///
/// It does not render itself, and just holds CSS stylesheet information for the rest of
/// the code to use.
pub struct NodeStyle {
    type_: RefCell<Option<String>>,
}

impl NodeStyle {
    pub fn new() -> NodeStyle {
        NodeStyle {
            type_: RefCell::new(None),
        }
    }

    pub fn get_css(&self, node: &RsvgNode) -> String {
        // FIXME: See these:
        //
        // https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute
        // https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute
        //
        // If the "type" attribute is not present, we should fallback to the
        // "contentStyleType" attribute of the svg element, which in turn
        // defaults to "text/css".

        let have_css = self
            .type_
            .borrow()
            .as_ref()
            .map(|t| t == "text/css")
            .unwrap_or(true);

        if have_css {
            node.children()
                .into_iter()
                .filter_map(|child| {
                    if child.get_type() == NodeType::Chars {
                        Some(child.with_impl(|chars: &NodeChars| chars.get_string()))
                    } else {
                        None
                    }
                })
                .collect::<String>()
        } else {
            "".to_string()
        }
    }
}

impl NodeTrait for NodeStyle {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            if attr == Attribute::Type {
                *self.type_.borrow_mut() = Some(value.to_string());
            }
        }

        Ok(())
    }
}
