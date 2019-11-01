use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::property_bag::PropertyBag;
use crate::text::NodeChars;

/// Represents a <style> node.
///
/// It does not render itself, and just holds CSS stylesheet information for the rest of
/// the code to use.
#[derive(Default)]
pub struct Style {
    type_: Option<String>,
}

impl Style {
    pub fn get_css(&self, node: &RsvgNode) -> String {
        // FIXME: See these:
        //
        // https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute
        // https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute
        //
        // If the "type" attribute is not present, we should fallback to the
        // "contentStyleType" attribute of the svg element, which in turn
        // defaults to "text/css".

        let have_css = self.type_.as_ref().map(|t| t == "text/css").unwrap_or(true);

        if have_css {
            node.children()
                .filter_map(|child| {
                    if child.borrow().get_type() == NodeType::Chars {
                        Some(child.borrow().get_impl::<NodeChars>().get_string())
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

impl NodeTrait for Style {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            if attr.expanded() == expanded_name!(svg "type") {
                self.type_ = Some(value.to_string());
            }
        }

        Ok(())
    }
}
