//! The <filter> node.
use std::cell::Cell;

use cairo::{self, MatrixTrait};
use markup5ever::local_name;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::error::ValueErrorKind;
use crate::length::{LengthHorizontal, LengthUnit, LengthVertical};
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{Parse, ParseError, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;

/// The <filter> node.
pub struct NodeFilter {
    pub x: Cell<LengthHorizontal>,
    pub y: Cell<LengthVertical>,
    pub width: Cell<LengthHorizontal>,
    pub height: Cell<LengthVertical>,
    pub filterunits: Cell<CoordUnits>,
    pub primitiveunits: Cell<CoordUnits>,
}

impl Default for NodeFilter {
    /// Constructs a new `NodeFilter` with default properties.
    #[inline]
    fn default() -> Self {
        Self {
            x: Cell::new(LengthHorizontal::parse_str("-10%").unwrap()),
            y: Cell::new(LengthVertical::parse_str("-10%").unwrap()),
            width: Cell::new(LengthHorizontal::parse_str("120%").unwrap()),
            height: Cell::new(LengthVertical::parse_str("120%").unwrap()),
            filterunits: Cell::new(CoordUnits::ObjectBoundingBox),
            primitiveunits: Cell::new(CoordUnits::UserSpaceOnUse),
        }
    }
}

impl NodeFilter {
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
        let params = if self.filterunits.get() == CoordUnits::ObjectBoundingBox {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        // With filterunits == ObjectBoundingBox, lengths represent fractions or percentages of the
        // referencing node. No units are allowed (it's checked during attribute parsing).
        let rect = if self.filterunits.get() == CoordUnits::ObjectBoundingBox {
            cairo::Rectangle {
                x: self.x.get().get_unitless(),
                y: self.y.get().get_unitless(),
                width: self.width.get().get_unitless(),
                height: self.height.get().get_unitless(),
            }
        } else {
            cairo::Rectangle {
                x: self.x.get().normalize(values, &params),
                y: self.y.get().normalize(values, &params),
                width: self.width.get().normalize(values, &params),
                height: self.height.get().normalize(values, &params),
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

impl NodeTrait for NodeFilter {
    fn set_atts(&self, _node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        // Parse filterUnits first as it affects x, y, width, height checks.
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("filterUnits") => self.filterunits.set(attr.parse(value)?),
                _ => (),
            }
        }

        // With ObjectBoundingBox, only fractions and percents are allowed.
        let no_units_allowed = self.filterunits.get() == CoordUnits::ObjectBoundingBox;

        let check_units_horizontal = |length: LengthHorizontal| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit() {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
            }
        };

        let check_units_vertical = |length: LengthVertical| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit() {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with filterUnits set to objectBoundingBox",
                ))),
            }
        };

        let check_units_horizontal_and_ensure_nonnegative = |length: LengthHorizontal| {
            check_units_horizontal(length).and_then(LengthHorizontal::check_nonnegative)
        };

        let check_units_vertical_and_ensure_nonnegative = |length: LengthVertical| {
            check_units_vertical(length).and_then(LengthVertical::check_nonnegative)
        };

        // Parse the rest of the attributes.
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("x") => self
                    .x
                    .set(attr.parse_and_validate(value, check_units_horizontal)?),
                local_name!("y") => self
                    .y
                    .set(attr.parse_and_validate(value, check_units_vertical)?),
                local_name!("width") => self.width.set(
                    attr.parse_and_validate(value, check_units_horizontal_and_ensure_nonnegative)?,
                ),
                local_name!("height") => self.height.set(
                    attr.parse_and_validate(value, check_units_vertical_and_ensure_nonnegative)?,
                ),
                local_name!("primitiveUnits") => self.primitiveunits.set(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }
}
