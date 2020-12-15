//! The `filter` element.

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::slice::Iter;

use crate::attributes::Attributes;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeId};
use crate::drawing_ctx::ViewParams;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::ValueErrorKind;
use crate::length::*;
use crate::node::{Node, NodeBorrow};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::rect::Rect;

/// The <filter> node.
pub struct Filter {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: Length<Horizontal>,
    height: Length<Vertical>,
    filterunits: CoordUnits,
    primitiveunits: CoordUnits,
}

impl Default for Filter {
    /// Constructs a new `Filter` with default properties.
    fn default() -> Self {
        Self {
            x: Length::<Horizontal>::parse_str("-10%").unwrap(),
            y: Length::<Vertical>::parse_str("-10%").unwrap(),
            width: Length::<Horizontal>::parse_str("120%").unwrap(),
            height: Length::<Vertical>::parse_str("120%").unwrap(),
            filterunits: CoordUnits::ObjectBoundingBox,
            primitiveunits: CoordUnits::UserSpaceOnUse,
        }
    }
}

impl Filter {
    pub fn get_filter_units(&self) -> CoordUnits {
        self.filterunits
    }

    pub fn get_primitive_units(&self) -> CoordUnits {
        self.primitiveunits
    }

    pub fn get_rect(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        // With filterunits == ObjectBoundingBox, lengths represent fractions or percentages of the
        // referencing node. No units are allowed (it's checked during attribute parsing).
        let (x, y, w, h) = if self.filterunits == CoordUnits::ObjectBoundingBox {
            (
                self.x.length,
                self.y.length,
                self.width.length,
                self.height.length,
            )
        } else {
            (
                self.x.normalize(values, &params),
                self.y.normalize(values, &params),
                self.width.normalize(values, &params),
                self.height.normalize(values, &params),
            )
        };

        Rect::new(x, y, x + w, y + h)
    }
}

impl SetAttributes for Filter {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        // Parse filterUnits first as it affects x, y, width, height checks.
        let result = attrs
            .iter()
            .find(|(attr, _)| attr.expanded() == expanded_name!("", "filterUnits"))
            .and_then(|(attr, value)| attr.parse(value).ok());
        if let Some(filter_units) = result {
            self.filterunits = filter_units
        }

        // With ObjectBoundingBox, only fractions and percents are allowed.
        let no_units_allowed = self.filterunits == CoordUnits::ObjectBoundingBox;

        let check_units_horizontal = |length: Length<Horizontal>| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::parse_error(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                )),
            }
        };

        let check_units_vertical = |length: Length<Vertical>| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::parse_error(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                )),
            }
        };

        let check_units_horizontal_and_ensure_nonnegative = |length: Length<Horizontal>| {
            check_units_horizontal(length).and_then(Length::<Horizontal>::check_nonnegative)
        };

        let check_units_vertical_and_ensure_nonnegative = |length: Length<Vertical>| {
            check_units_vertical(length).and_then(Length::<Vertical>::check_nonnegative)
        };

        // Parse the rest of the attributes.
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => {
                    self.x = attr.parse_and_validate(value, check_units_horizontal)?
                }
                expanded_name!("", "y") => {
                    self.y = attr.parse_and_validate(value, check_units_vertical)?
                }
                expanded_name!("", "width") => {
                    self.width = attr
                        .parse_and_validate(value, check_units_horizontal_and_ensure_nonnegative)?
                }
                expanded_name!("", "height") => {
                    self.height =
                        attr.parse_and_validate(value, check_units_vertical_and_ensure_nonnegative)?
                }
                expanded_name!("", "primitiveUnits") => self.primitiveunits = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Filter {}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
    Url(NodeId),
    // TODO: add functions from https://www.w3.org/TR/filter-effects-1/#filter-functions
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterValueList(Vec<FilterValue>);

impl Default for FilterValueList {
    fn default() -> FilterValueList {
        FilterValueList(vec![])
    }
}

impl FilterValueList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> Iter<'_, FilterValue> {
        self.0.iter()
    }

    /// Check that at least one filter URI exists and that all contained
    /// URIs reference existing <filter> elements.
    pub fn is_applicable(&self, node: &Node, acquired_nodes: &mut AcquiredNodes<'_>) -> bool {
        if self.is_empty() {
            return false;
        }

        self.iter()
            .map(|v| match v {
                FilterValue::Url(v) => v,
            })
            .all(|v| match acquired_nodes.acquire(v) {
                Ok(acquired) => {
                    let filter_node = acquired.get();

                    match *filter_node.borrow_element() {
                        Element::Filter(_) => true,
                        _ => {
                            rsvg_log!(
                                "element {} will not be filtered since \"{}\" is not a filter",
                                node,
                                v,
                            );
                            false
                        }
                    }
                }
                _ => {
                    rsvg_log!(
                        "element {} will not be filtered since its filter \"{}\" was not found",
                        node,
                        v,
                    );
                    false
                }
            })
    }
}

impl Parse for FilterValueList {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, crate::error::ParseError<'i>> {
        let mut result = FilterValueList::default();

        loop {
            let loc = parser.current_source_location();

            let url = parser.expect_url()?;
            let node_id =
                NodeId::parse(&url).map_err(|e| loc.new_custom_error(ValueErrorKind::from(e)))?;
            result.0.push(FilterValue::Url(node_id));

            if parser.is_exhausted() {
                break;
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filter_value_list() {
        let n1 = NodeId::External("foo.svg".to_string(), "bar".to_string());
        let n2 = NodeId::External("test.svg".to_string(), "baz".to_string());
        assert_eq!(
            FilterValueList::parse_str("url(foo.svg#bar) url(test.svg#baz)").unwrap(),
            FilterValueList(vec![FilterValue::Url(n1), FilterValue::Url(n2)])
        );
    }

    #[test]
    fn detects_invalid_filter_value_list() {
        assert!(FilterValueList::parse_str("none").is_err());
        assert!(FilterValueList::parse_str("").is_err());
        assert!(FilterValueList::parse_str("fail").is_err());
        assert!(FilterValueList::parse_str("url(#test) none").is_err());
    }
}
