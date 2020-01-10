//! Entry point for the CSS filters infrastructure.

use std::ops::Deref;
use std::time::Instant;

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::error::{RenderingError, ValueErrorKind};
use crate::filter::Filter;
use crate::length::*;
use crate::node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::parsers::ParseValue;
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::ColorInterpolationFilters;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

mod bounds;
use self::bounds::BoundsBuilder;

pub mod context;
use self::context::{FilterContext, FilterInput, FilterResult};

mod error;
use self::error::FilterError;

mod input;
use self::input::{CustomIdent, Input};

/// A filter primitive interface.
pub trait FilterEffect: NodeTrait {
    /// Renders this filter primitive.
    ///
    /// If this filter primitive can't be rendered for whatever reason (for instance, a required
    /// property hasn't been provided), an error is returned.
    fn render(
        &self,
        node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError>;

    /// Returns `true` if this filter primitive is affected by the `color-interpolation-filters`
    /// property.
    ///
    /// Primitives that do color blending (like `feComposite` or `feBlend`) should return `true`
    /// here, whereas primitives that don't (like `feOffset`) should return `false`.
    fn is_affected_by_color_interpolation_filters(&self) -> bool;
}

macro_rules! impl_node_as_filter_effect {
    () => (
        fn as_filter_effect(&self) -> Option<&dyn FilterEffect> {
            Some(self)
        }
    )
}

pub mod blend;
pub mod color_matrix;
pub mod component_transfer;
pub mod composite;
pub mod convolve_matrix;
pub mod displacement_map;
pub mod flood;
pub mod gaussian_blur;
pub mod image;
pub mod light;
pub mod merge;
pub mod morphology;
pub mod offset;
pub mod tile;
pub mod turbulence;

/// The base filter primitive node containing common properties.
struct Primitive {
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    width: Option<Length<Horizontal>>,
    height: Option<Length<Vertical>>,
    result: Option<CustomIdent>,
}

/// The base node for filter primitives which accept input.
struct PrimitiveWithInput {
    base: Primitive,
    in_: Option<Input>,
}

impl Primitive {
    /// Constructs a new `Primitive` with empty properties.
    #[inline]
    fn new<T: FilterEffect>() -> Primitive {
        Primitive {
            x: None,
            y: None,
            width: None,
            height: None,
            result: None,
        }
    }

    /// Returns the `BoundsBuilder` for bounds computation.
    #[inline]
    fn get_bounds<'a>(&self, ctx: &'a FilterContext) -> BoundsBuilder<'a> {
        BoundsBuilder::new(ctx, self.x, self.y, self.width, self.height)
    }
}

impl NodeTrait for Primitive {
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        // With ObjectBoundingBox, only fractions and percents are allowed.
        let primitiveunits = parent
            .and_then(|parent| {
                if parent.borrow().get_type() == NodeType::Filter {
                    Some(parent.borrow().get_impl::<Filter>().get_primitive_units())
                } else {
                    None
                }
            })
            .unwrap_or(CoordUnits::UserSpaceOnUse);

        let no_units_allowed = primitiveunits == CoordUnits::ObjectBoundingBox;

        let check_units_horizontal = |length: Length<Horizontal>| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Px | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::parse_error(
                    "unit identifiers are not allowed with primitiveUnits set to objectBoundingBox",
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
                    "unit identifiers are not allowed with primitiveUnits set to objectBoundingBox",
                )),
            }
        };

        let check_units_horizontal_and_ensure_nonnegative = |length: Length<Horizontal>| {
            check_units_horizontal(length).and_then(Length::<Horizontal>::check_nonnegative)
        };

        let check_units_vertical_and_ensure_nonnegative = |length: Length<Vertical>| {
            check_units_vertical(length).and_then(Length::<Vertical>::check_nonnegative)
        };

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => {
                    self.x = Some(attr.parse_and_validate(value, check_units_horizontal)?)
                }
                expanded_name!(svg "y") => {
                    self.y = Some(attr.parse_and_validate(value, check_units_vertical)?)
                }
                expanded_name!(svg "width") => {
                    self.width =
                        Some(attr.parse_and_validate(
                            value,
                            check_units_horizontal_and_ensure_nonnegative,
                        )?)
                }
                expanded_name!(svg "height") => {
                    self.height =
                        Some(attr.parse_and_validate(
                            value,
                            check_units_vertical_and_ensure_nonnegative,
                        )?)
                }
                expanded_name!(svg "result") => self.result = Some(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }
}

impl PrimitiveWithInput {
    /// Constructs a new `PrimitiveWithInput` with empty properties.
    #[inline]
    fn new<T: FilterEffect>() -> PrimitiveWithInput {
        PrimitiveWithInput {
            base: Primitive::new::<T>(),
            in_: None,
        }
    }

    /// Returns the input Cairo surface for this filter primitive.
    #[inline]
    fn get_input(
        &self,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterInput, FilterError> {
        ctx.get_input(draw_ctx, self.in_.as_ref())
    }
}

impl NodeTrait for PrimitiveWithInput {
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "in") => self.in_ = Some(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }
}

impl Deref for PrimitiveWithInput {
    type Target = Primitive;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/// Applies a filter and returns the resulting surface.
pub fn render(
    filter_node: &RsvgNode,
    computed_from_node_being_filtered: &ComputedValues,
    source: &cairo::ImageSurface,
    draw_ctx: &mut DrawingCtx,
    node_bbox: BoundingBox,
) -> Result<cairo::ImageSurface, RenderingError> {
    let filter_node = &*filter_node;
    assert_eq!(filter_node.borrow().get_type(), NodeType::Filter);
    assert!(!filter_node.borrow().is_in_error());

    // The source surface has multiple references. We need to copy it to a new surface to have a
    // unique reference to be able to safely access the pixel data.
    let source_surface = SharedImageSurface::copy_from_surface(source)?;

    let mut filter_ctx = FilterContext::new(
        filter_node,
        computed_from_node_being_filtered,
        source_surface,
        draw_ctx,
        node_bbox,
    );

    // If paffine is non-invertible, we won't draw anything. Also bbox combining in bounds
    // computations will panic due to non-invertible martrix.
    if filter_ctx.paffine().try_invert().is_err() {
        return Ok(filter_ctx.into_output()?.into_image_surface()?);
    }

    let primitives = filter_node
        .children()
        // Skip nodes in error.
        .filter(|c| {
            let in_error = c.borrow().is_in_error();

            if in_error {
                rsvg_log!("(ignoring filter primitive {} because it is in error)", c);
            }

            !in_error
        })
        // Keep only filter primitives (those that implement the Filter trait)
        .filter(|c| c.borrow().get_node_trait().as_filter_effect().is_some())
        // Check if the node wants linear RGB.
        .map(|c| {
            let linear_rgb = {
                let cascaded = CascadedValues::new_from_node(&c);
                let values = cascaded.get();

                values.color_interpolation_filters == ColorInterpolationFilters::LinearRgb
            };

            (c, linear_rgb)
        });

    for (c, linear_rgb) in primitives {
        let node_data = c.borrow();
        let filter = node_data.get_node_trait().as_filter_effect().unwrap();

        let mut render = |filter_ctx: &mut FilterContext| {
            if let Err(err) = filter
                .render(&c, filter_ctx, draw_ctx)
                .and_then(|result| filter_ctx.store_result(result))
            {
                rsvg_log!("(filter primitive {} returned an error: {})", c, err);

                // Exit early on Cairo errors. Continue rendering otherwise.
                if let FilterError::CairoError(status) = err {
                    return Err(status);
                }
            }

            Ok(())
        };

        let start = Instant::now();

        if filter.is_affected_by_color_interpolation_filters() && linear_rgb {
            filter_ctx.with_linear_rgb(render)?;
        } else {
            render(&mut filter_ctx)?;
        }

        let elapsed = start.elapsed();
        rsvg_log!(
            "(rendered filter primitive {} in\n    {} seconds)",
            c,
            elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) / 1e9
        );
    }

    Ok(filter_ctx.into_output()?.into_image_surface()?)
}

impl From<ColorInterpolationFilters> for SurfaceType {
    fn from(c: ColorInterpolationFilters) -> Self {
        match c {
            ColorInterpolationFilters::LinearRgb => SurfaceType::LinearRgb,
            _ => SurfaceType::SRgb,
        }
    }
}
