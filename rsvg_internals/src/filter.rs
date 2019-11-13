use cairo;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::error::ValueErrorKind;
use crate::length::*;
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseError, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;

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
        affine: cairo::Matrix,
        width: f64,
        height: f64,
    ) -> BoundingBox {
        // Filters use the properties of the target node.
        let values = computed_from_target_node;

        let mut bbox = BoundingBox::new(&cairo::Matrix::identity());

        // affine is set up in FilterContext::new() in such a way that for
        // filterunits == ObjectBoundingBox affine includes scaling to correct width, height and
        // this is why width and height are set to 1, 1 (and for filterunits ==
        // UserSpaceOnUse affine doesn't include scaling because in this case the correct
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
        let rect = if self.filterunits == CoordUnits::ObjectBoundingBox {
            cairo::Rectangle {
                x: self.x.length,
                y: self.y.length,
                width: self.width.length,
                height: self.height.length,
            }
        } else {
            cairo::Rectangle {
                x: self.x.normalize(values, &params),
                y: self.y.normalize(values, &params),
                width: self.width.normalize(values, &params),
                height: self.height.normalize(values, &params),
            }
        };

        let other_bbox = BoundingBox::new(&affine).with_rect(Some(rect));

        // At this point all of the previous viewbox and matrix business gets converted to pixel
        // coordinates in the final surface, because bbox is created with an identity affine.
        bbox.insert(&other_bbox);

        // Finally, clip to the width and height of our surface.
        let rect = cairo::Rectangle {
            x: 0f64,
            y: 0f64,
            width,
            height,
        };
        let other_bbox = BoundingBox::new(&cairo::Matrix::identity()).with_rect(Some(rect));
        bbox.clip(&other_bbox);

        bbox
    }
}

impl NodeTrait for Filter {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        // Parse filterUnits first as it affects x, y, width, height checks.
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "filterUnits") => self.filterunits = attr.parse(value)?,
                _ => (),
            }
        }

        // With ObjectBoundingBox, only fractions and percents are allowed.
        let no_units_allowed = self.filterunits == CoordUnits::ObjectBoundingBox;

        let check_units_horizontal = |length: Length<Horizontal>| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
            }
        };

        let check_units_vertical = |length: Length<Vertical>| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
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
                expanded_name!(svg "x") => {
                    self.x = attr.parse_and_validate(value, check_units_horizontal)?
                }
                expanded_name!(svg "y") => {
                    self.y = attr.parse_and_validate(value, check_units_vertical)?
                }
                expanded_name!(svg "width") => {
                    self.width = attr
                        .parse_and_validate(value, check_units_horizontal_and_ensure_nonnegative)?
                }
                expanded_name!(svg "height") => {
                    self.height =
                        attr.parse_and_validate(value, check_units_vertical_and_ensure_nonnegative)?
                }
                expanded_name!(svg "primitiveUnits") => self.primitiveunits = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}
