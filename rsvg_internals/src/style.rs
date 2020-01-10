//! The `style` element.

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::error::*;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;

/// Represents the syntax used in the <style> node.
///
/// Currently only "text/css" is supported.
///
/// https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute
/// https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute
#[derive(Copy, Clone, PartialEq)]
pub enum StyleType {
    TextCss,
}

impl Parse for StyleType {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<StyleType, CssParseError<'i>> {
        parser.expect_ident_matching("text/css")?;
        Ok(StyleType::TextCss)
    }
}

/// Represents a <style> node.
///
/// It does not render itself, and just holds CSS stylesheet information for the rest of
/// the code to use.
#[derive(Default)]
pub struct Style {
    type_: Option<StyleType>,
}

impl Style {
    pub fn style_type(&self) -> Option<StyleType> {
        self.type_
    }
}

impl NodeTrait for Style {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            if attr.expanded() == expanded_name!("", "type") {
                self.type_ = Some(attr.parse(value)?);
            }
        }

        Ok(())
    }
}
