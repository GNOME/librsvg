use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::time::Instant;

use cairo::{self, MatrixTrait};
use owning_ref::RcRef;

use attributes::Attribute;
use coord_units::CoordUnits;
use drawing_ctx::DrawingCtx;
use error::{RenderingError, ValueErrorKind};
use handle::RsvgHandle;
use length::{Length, LengthDir, LengthUnit};
use node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use parsers::{parse_and_validate, ParseError};
use property_bag::PropertyBag;
use state::{ColorInterpolationFilters, ComputedValues};
use surface_utils::shared_surface::{SharedImageSurface, SurfaceType};

mod bounds;
use self::bounds::BoundsBuilder;

pub mod context;
use self::context::{FilterContext, FilterInput, FilterResult};

mod error;
use self::error::FilterError;

mod input;
use self::input::Input;

pub mod node;
use self::node::NodeFilter;

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

/// A filter primitive interface.
trait Filter: NodeTrait {
    /// Renders this filter primitive.
    ///
    /// If this filter primitive can't be rendered for whatever reason (for instance, a required
    /// property hasn't been provided), an error is returned.
    fn render(
        &self,
        node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError>;

    /// Returns `true` if this filter primitive is affected by the `color-interpolation-filters`
    /// property.
    ///
    /// Primitives that do color blending (like `feComposite` or `feBlend`) should return `true`
    /// here, whereas primitives that don't (like `feOffset`) should return `false`.
    fn is_affected_by_color_interpolation_filters(&self) -> bool;
}

/// The base filter primitive node containing common properties.
struct Primitive {
    x: Cell<Option<Length>>,
    y: Cell<Option<Length>>,
    width: Cell<Option<Length>>,
    height: Cell<Option<Length>>,
    result: RefCell<Option<String>>,
}

/// The base node for filter primitives which accept input.
struct PrimitiveWithInput {
    base: Primitive,
    in_: RefCell<Option<Input>>,
}

impl Primitive {
    /// Constructs a new `Primitive` with empty properties.
    #[inline]
    fn new<T: Filter>() -> Primitive {
        Primitive {
            x: Cell::new(None),
            y: Cell::new(None),
            width: Cell::new(None),
            height: Cell::new(None),
            result: RefCell::new(None),
        }
    }

    /// Returns the `BoundsBuilder` for bounds computation.
    #[inline]
    fn get_bounds<'a>(&self, ctx: &'a FilterContext) -> BoundsBuilder<'a> {
        BoundsBuilder::new(
            ctx,
            self.x.get(),
            self.y.get(),
            self.width.get(),
            self.height.get(),
        )
    }
}

impl NodeTrait for Primitive {
    fn set_atts(
        &self,
        node: &RsvgNode,
        _: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        // With ObjectBoundingBox, only fractions and percents are allowed.
        let primitiveunits = node
            .get_parent()
            .and_then(|parent| {
                if parent.get_type() == NodeType::Filter {
                    Some(parent.with_impl(|f: &NodeFilter| f.primitiveunits.get()))
                } else {
                    None
                }
            })
            .unwrap_or(CoordUnits::UserSpaceOnUse);

        let no_units_allowed = primitiveunits == CoordUnits::ObjectBoundingBox;
        let check_units = |length: Length| {
            if !no_units_allowed {
                return Ok(length);
            }

            match length.unit {
                LengthUnit::Default | LengthUnit::Percent => Ok(length),
                _ => Err(ValueErrorKind::Parse(ParseError::new(
                    "unit identifiers are not allowed with primitiveUnits set to objectBoundingBox",
                ))),
            }
        };
        let check_units_and_ensure_nonnegative =
            |length: Length| check_units(length).and_then(Length::check_nonnegative);

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(Some(parse_and_validate(
                    "x",
                    value,
                    LengthDir::Horizontal,
                    check_units,
                )?)),
                Attribute::Y => self.y.set(Some(parse_and_validate(
                    "y",
                    value,
                    LengthDir::Vertical,
                    check_units,
                )?)),
                Attribute::Width => self.width.set(Some(parse_and_validate(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    check_units_and_ensure_nonnegative,
                )?)),
                Attribute::Height => self.height.set(Some(parse_and_validate(
                    "height",
                    value,
                    LengthDir::Vertical,
                    check_units_and_ensure_nonnegative,
                )?)),
                Attribute::Result => *self.result.borrow_mut() = Some(value.to_string()),
                _ => (),
            }
        }

        Ok(())
    }
}

impl PrimitiveWithInput {
    /// Constructs a new `PrimitiveWithInput` with empty properties.
    #[inline]
    fn new<T: Filter>() -> PrimitiveWithInput {
        PrimitiveWithInput {
            base: Primitive::new::<T>(),
            in_: RefCell::new(None),
        }
    }

    /// Returns the input Cairo surface for this filter primitive.
    #[inline]
    fn get_input(
        &self,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterInput, FilterError> {
        ctx.get_input(draw_ctx, self.in_.borrow().as_ref())
    }
}

impl NodeTrait for PrimitiveWithInput {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::In => drop(self.in_.replace(Some(Input::parse(Attribute::In, value)?))),
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
    draw_ctx: &mut DrawingCtx<'_>,
) -> Result<cairo::ImageSurface, RenderingError> {
    let filter_node = &*filter_node;
    assert_eq!(filter_node.get_type(), NodeType::Filter);
    assert!(!filter_node.is_in_error());

    // The source surface has multiple references. We need to copy it to a new surface to have a
    // unique reference to be able to safely access the pixel data.
    let source_surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        source.get_width(),
        source.get_height(),
    )?;
    {
        let cr = cairo::Context::new(&source_surface);
        cr.set_source_surface(source, 0f64, 0f64);
        cr.paint();
    }
    let source_surface = SharedImageSurface::new(source_surface, SurfaceType::SRgb)?;

    let mut filter_ctx = FilterContext::new(
        filter_node,
        computed_from_node_being_filtered,
        source_surface,
        draw_ctx,
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
            let in_error = c.is_in_error();

            if in_error {
                rsvg_log!(
                    "(ignoring filter primitive {} because it is in error)",
                    c.get_human_readable_name()
                );
            }

            !in_error
        })
        // Check if the node wants linear RGB.
        .map(|c| {
            let linear_rgb = {
                let cascaded = c.get_cascaded_values();
                let values = cascaded.get();

                values.color_interpolation_filters == ColorInterpolationFilters::LinearRgb
            };

            (c, linear_rgb)
        })
        .filter_map(|(c, linear_rgb)| {
            let rr = RcRef::new(c)
                .try_map(|c| {
                    // Go through the filter primitives and see if the node is one of them.
                    #[inline]
                    fn as_filter<T: Filter>(x: &T) -> &Filter {
                        x
                    }

                    // Unfortunately it's not possible to downcast to a trait object. If we could
                    // attach arbitrary data to nodes it would really help here. Previously that
                    // arbitrary data was in form of RsvgCNodeImpl, but that was heavily tuned for
                    // storing C pointers with all subsequent downsides.
                    macro_rules! try_downcasting_to_filter {
                        ($c:expr; $($t:ty),+$(,)*) => ({
                            let mut filter = None;
                            $(
                                filter = filter.or_else(|| $c.get_impl::<$t>().map(as_filter));
                            )+
                            filter
                        })
                    }

                    let filter = try_downcasting_to_filter!(c;
                        blend::Blend,
                        color_matrix::ColorMatrix,
                        component_transfer::ComponentTransfer,
                        composite::Composite,
                        convolve_matrix::ConvolveMatrix,
                        displacement_map::DisplacementMap,
                        flood::Flood,
                        gaussian_blur::GaussianBlur,
                        image::Image,
                        light::lighting::Lighting,
                        merge::Merge,
                        morphology::Morphology,
                        offset::Offset,
                        tile::Tile,
                        turbulence::Turbulence,
                    );
                    filter.ok_or(())
                })
                .ok();

            rr.map(|rr| (rr, linear_rgb))
        });

    for (rr, linear_rgb) in primitives {
        let mut render = |filter_ctx: &mut FilterContext| {
            if let Err(err) = rr
                .render(rr.owner(), filter_ctx, draw_ctx)
                .and_then(|result| filter_ctx.store_result(result))
            {
                rsvg_log!(
                    "(filter primitive {} returned an error: {})",
                    rr.owner().get_human_readable_name(),
                    err
                );

                // Exit early on Cairo errors. Continue rendering otherwise.
                if let FilterError::CairoError(status) = err {
                    return Err(status);
                }
            }

            Ok(())
        };

        let start = Instant::now();

        if rr.is_affected_by_color_interpolation_filters() && linear_rgb {
            filter_ctx.with_linear_rgb(render)?;
        } else {
            render(&mut filter_ctx)?;
        }

        let elapsed = start.elapsed();
        rsvg_log!(
            "(rendered filter primitive {} in\n    {} seconds)",
            rr.owner().get_human_readable_name(),
            elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) / 1e9
        );
    }

    Ok(filter_ctx.into_output()?.into_image_surface()?)
}
