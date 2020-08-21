//! The `filter` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::ValueErrorKind;
use crate::length::*;
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::rect::Rect;
use crate::transform::Transform;

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

    /// Computes and returns the filter effects region.
    pub fn compute_effects_region(
        &self,
        computed_from_target_node: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        transform: Transform,
        width: f64,
        height: f64,
    ) -> BoundingBox {
        // Filters use the properties of the target node.
        let values = computed_from_target_node;

        let mut bbox = BoundingBox::new();

        // transform is set up in FilterContext::new() in such a way that for
        // filterunits == ObjectBoundingBox it includes scaling to correct width, height and
        // this is why width and height are set to 1, 1 (and for filterunits ==
        // UserSpaceOnUse, transform doesn't include scaling because in this case the correct
        // width, height already happens to be the viewbox width, height).
        //
        // It's done this way because with ObjectBoundingBox, non-percentage values are supposed to
        // represent the fractions of the referenced node, and with width and height = 1, 1 this
        // works out exactly like that.
        let params = if self.filterunits == CoordUnits::ObjectBoundingBox {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

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

        let rect = Rect::new(x, y, x + w, y + h);
        let other_bbox = BoundingBox::new().with_transform(transform).with_rect(rect);

        // At this point all of the previous viewbox and matrix business gets converted to pixel
        // coordinates in the final surface, because bbox is created with an identity transform.
        bbox.insert(&other_bbox);

        // Finally, clip to the width and height of our surface.
        let rect = Rect::from_size(width, height);
        let other_bbox = BoundingBox::new().with_rect(rect);
        bbox.clip(&other_bbox);

        bbox
    }
}

impl SetAttributes for Filter {
    fn set_attributes(&mut self, pbag: &PropertyBag<'_>) -> ElementResult {
        // Parse filterUnits first as it affects x, y, width, height checks.
        let result = pbag
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
        for (attr, value) in pbag.iter() {
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
