use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::error::*;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseError, ParseValue};
use crate::property_bag::PropertyBag;

/// Represents the syntax used in the <style> node.
///
/// Currently only "text/css" is supported.
#[derive(Copy, Clone, PartialEq)]
pub enum StyleType {
    TextCss,
}

impl Parse for StyleType {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<StyleType, ValueErrorKind> {
        parser
            .expect_ident_matching("text/css")
            .and_then(|_| Ok(StyleType::TextCss))
            .map_err(|_| {
                ValueErrorKind::Parse(ParseError::new(
                    "only the \"text/css\" style type is supported",
                ))
            })
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
    pub fn style_type(&self) -> StyleType {
        // FIXME: See these:
        //
        // https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute
        // https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute
        //
        // If the "type" attribute is not present, we should fallback to the
        // "contentStyleType" attribute of the svg element, which in turn
        // defaults to "text/css".
        self.type_.unwrap_or(StyleType::TextCss)
    }
}

impl NodeTrait for Style {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            if attr.expanded() == expanded_name!(svg "type") {
                self.type_ = Some(attr.parse(value)?);
            }
        }

        Ok(())
    }
}
