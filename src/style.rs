//! The `style` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::*;
use crate::xml::Attributes;

/// Represents the syntax used in the <style> node.
///
/// Currently only "text/css" is supported.
///
/// <https://www.w3.org/TR/SVG11/styling.html#StyleElementTypeAttribute>
/// <https://www.w3.org/TR/SVG11/styling.html#ContentStyleTypeAttribute>
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum StyleType {
    TextCss,
}

enum_default!(StyleType, StyleType::TextCss);

impl StyleType {
    fn parse(value: &str) -> Result<StyleType, ValueErrorKind> {
        // https://html.spec.whatwg.org/multipage/semantics.html#the-style-element
        //
        // 4. If element's type attribute is present and its value is
        // neither the empty string nor an ASCII case-insensitive
        // match for "text/css", then return.

        if value.eq_ignore_ascii_case("text/css") {
            Ok(StyleType::TextCss)
        } else {
            Err(ValueErrorKind::parse_error(
                "invalid value for type attribute in style element",
            ))
        }
    }
}

/// Represents a <style> node.
///
/// It does not render itself, and just holds CSS stylesheet information for the rest of
/// the code to use.
#[derive(Default)]
pub struct Style {
    type_: StyleType,
}

impl Style {
    pub fn style_type(&self) -> StyleType {
        self.type_
    }
}

impl SetAttributes for Style {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            if attr.expanded() == expanded_name!("", "type") {
                self.type_ = StyleType::parse(value).attribute(attr)?;
            }
        }

        Ok(())
    }
}

impl Draw for Style {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_style_type() {
        assert_eq!(StyleType::parse("text/css").unwrap(), StyleType::TextCss);
    }

    #[test]
    fn invalid_style_type_yields_error() {
        assert!(StyleType::parse("").is_err());
        assert!(StyleType::parse("some-other-stylesheet-language").is_err());
    }
}
