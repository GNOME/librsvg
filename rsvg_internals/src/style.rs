use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::property_bag::PropertyBag;

/// Represents a <style> node.
///
/// It does not render itself, and just holds CSS stylesheet information for the rest of
/// the code to use.
#[derive(Default)]
pub struct Style {
    type_: Option<String>,
}

impl Style {
    pub fn is_text_css(&self) -> bool {
        // FIXME: See these:
        //
        // https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute
        // https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute
        //
        // If the "type" attribute is not present, we should fallback to the
        // "contentStyleType" attribute of the svg element, which in turn
        // defaults to "text/css".

        self.type_.as_ref().map(|t| t == "text/css").unwrap_or(true)
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
