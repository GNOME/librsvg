//! Entry point for the CSS filters infrastructure.

use cssparser::{BasicParseError, Parser};
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::time::Instant;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::{ElementError, ParseError, RenderingError};
use crate::length::*;
use crate::node::{Node, NodeBorrow};
use crate::paint_server::UserSpacePaintSource;
use crate::parsers::{CustomIdent, Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::property_defs::ColorInterpolationFilters;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
use crate::transform::Transform;
use crate::xml::Attributes;

mod bounds;
use self::bounds::BoundsBuilder;

pub mod context;
use self::context::{FilterContext, FilterOutput, FilterResult};

mod error;
use self::error::FilterError;

/// A filter primitive interface.
pub trait FilterEffect: SetAttributes + Draw {
    fn resolve(&self, node: &Node) -> Result<ResolvedPrimitive, FilterError>;
}

// Filter Effects do not need to draw themselves
impl<T: FilterEffect> Draw for T {}

pub mod blend;
pub mod color_matrix;
pub mod component_transfer;
pub mod composite;
pub mod convolve_matrix;
pub mod displacement_map;
pub mod flood;
pub mod gaussian_blur;
pub mod image;
pub mod lighting;
pub mod merge;
pub mod morphology;
pub mod offset;
pub mod tile;
pub mod turbulence;

/// Resolved parameters for each filter primitive.
///
/// These gather all the data that a primitive may need during rendering:
/// the `feFoo` element's attributes, any computed values from its properties,
/// and parameters extracted from the element's children (for example,
/// `feMerge` gathers info from its `feMergNode` children).
pub enum PrimitiveParams {
    Blend(blend::Blend),
    ColorMatrix(color_matrix::ColorMatrix),
    ComponentTransfer(component_transfer::ComponentTransfer),
    Composite(composite::Composite),
    ConvolveMatrix(convolve_matrix::ConvolveMatrix),
    DiffuseLighting(lighting::DiffuseLighting),
    DisplacementMap(displacement_map::DisplacementMap),
    Flood(flood::Flood),
    GaussianBlur(gaussian_blur::GaussianBlur),
    Image(image::Image),
    Merge(merge::Merge),
    Morphology(morphology::Morphology),
    Offset(offset::Offset),
    SpecularLighting(lighting::SpecularLighting),
    Tile(tile::Tile),
    Turbulence(turbulence::Turbulence),
}

impl PrimitiveParams {
    /// Returns a human-readable name for a primitive.
    #[rustfmt::skip]
    fn name(&self) -> &'static str {
        use PrimitiveParams::*;
        match self {
            Blend(..)             => "feBlend",
            ColorMatrix(..)       => "feColorMatrix",
            ComponentTransfer(..) => "feComponentTransfer",
            Composite(..)         => "feComposite",
            ConvolveMatrix(..)    => "feConvolveMatrix",
            DiffuseLighting(..)   => "feDiffuseLighting",
            DisplacementMap(..)   => "feDisplacementMap",
            Flood(..)             => "feFlood",
            GaussianBlur(..)      => "feGaussianBlur",
            Image(..)             => "feImage",
            Merge(..)             => "feMerge",
            Morphology(..)        => "feMorphology",
            Offset(..)            => "feOffset",
            SpecularLighting(..)  => "feSpecularLighting",
            Tile(..)              => "feTile",
            Turbulence(..)        => "feTurbulence",
        }
    }
}

/// The base filter primitive node containing common properties.
#[derive(Default, Clone)]
pub struct Primitive {
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    width: Option<ULength<Horizontal>>,
    height: Option<ULength<Vertical>>,
    result: Option<CustomIdent>,
}

pub struct ResolvedPrimitive {
    pub primitive: Primitive,
    pub params: PrimitiveParams,
}

/// A fully resolved filter primitive in user-space coordinates.
pub struct UserSpacePrimitive {
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    result: Option<CustomIdent>,

    params: PrimitiveParams,
}

/// An enumeration of possible inputs for a filter primitive.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Input {
    Unspecified,
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    FilterOutput(CustomIdent),
}

enum_default!(Input, Input::Unspecified);

impl Parse for Input {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        parser
            .try_parse(|p| {
                parse_identifiers!(
                    p,
                    "SourceGraphic" => Input::SourceGraphic,
                    "SourceAlpha" => Input::SourceAlpha,
                    "BackgroundImage" => Input::BackgroundImage,
                    "BackgroundAlpha" => Input::BackgroundAlpha,
                    "FillPaint" => Input::FillPaint,
                    "StrokePaint" => Input::StrokePaint,
                )
            })
            .or_else(|_: BasicParseError<'_>| {
                let ident = CustomIdent::parse(parser)?;
                Ok(Input::FilterOutput(ident))
            })
    }
}

impl ResolvedPrimitive {
    fn to_user_space(
        self,
        primitive_units: CoordUnits,
        values: &ComputedValues,
        draw_ctx: &DrawingCtx,
    ) -> UserSpacePrimitive {
        let params = draw_ctx.push_coord_units(primitive_units);

        let x = self.primitive.x.map(|l| l.normalize(values, &params));
        let y = self.primitive.y.map(|l| l.normalize(values, &params));
        let width = self.primitive.width.map(|l| l.normalize(values, &params));
        let height = self.primitive.height.map(|l| l.normalize(values, &params));

        UserSpacePrimitive {
            x,
            y,
            width,
            height,
            result: self.primitive.result,
            params: self.params,
        }
    }
}

impl UserSpacePrimitive {
    /// Validates attributes and returns the `BoundsBuilder` for bounds computation.
    #[inline]
    fn get_bounds(&self, ctx: &FilterContext) -> BoundsBuilder {
        BoundsBuilder::new(self.x, self.y, self.width, self.height, ctx.paffine())
    }
}

impl Primitive {
    fn parse_standard_attributes(
        &mut self,
        attrs: &Attributes,
    ) -> Result<(Input, Input), ElementError> {
        let mut input_1 = Input::Unspecified;
        let mut input_2 = Input::Unspecified;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => self.width = attr.parse(value)?,
                expanded_name!("", "height") => self.height = attr.parse(value)?,
                expanded_name!("", "result") => self.result = attr.parse(value)?,
                expanded_name!("", "in") => input_1 = attr.parse(value)?,
                expanded_name!("", "in2") => input_2 = attr.parse(value)?,
                _ => (),
            }
        }

        Ok((input_1, input_2))
    }

    pub fn parse_no_inputs(&mut self, attrs: &Attributes) -> ElementResult {
        let (_, _) = self.parse_standard_attributes(attrs)?;
        Ok(())
    }

    pub fn parse_one_input(&mut self, attrs: &Attributes) -> Result<Input, ElementError> {
        let (input_1, _) = self.parse_standard_attributes(attrs)?;
        Ok(input_1)
    }

    pub fn parse_two_inputs(&mut self, attrs: &Attributes) -> Result<(Input, Input), ElementError> {
        self.parse_standard_attributes(attrs)
    }
}

/// Applies a filter and returns the resulting surface.
pub fn render(
    filter_node: &Node,
    stroke_paint_source: UserSpacePaintSource,
    fill_paint_source: UserSpacePaintSource,
    source_surface: SharedImageSurface,
    acquired_nodes: &mut AcquiredNodes<'_>,
    draw_ctx: &mut DrawingCtx,
    transform: Transform,
    node_bbox: BoundingBox,
) -> Result<SharedImageSurface, RenderingError> {
    let filter_node = &*filter_node;
    assert!(is_element_of_type!(filter_node, Filter));

    let filter_element = filter_node.borrow_element();

    if filter_element.is_in_error() {
        return Ok(source_surface);
    }

    let user_space_filter = {
        let filter_values = filter_element.get_computed_values();

        let filter = borrow_element_as!(filter_node, Filter);

        // This is in a temporary scope so we don't leave the coord_units pushed during
        // the execution of all the filter primitives.
        let params = draw_ctx.push_coord_units(filter.get_filter_units());
        filter.to_user_space(filter_values, &params)
    };

    let res = filter_node
        .children()
        .filter(|c| c.is_element())
        // Skip nodes in error.
        .filter(|c| {
            let in_error = c.borrow_element().is_in_error();

            if in_error {
                rsvg_log!("(ignoring filter primitive {} because it is in error)", c);
            }

            !in_error
        })
        // Keep only filter primitives (those that implement the Filter trait)
        .filter(|c| c.borrow_element().as_filter_effect().is_some())
        .map(|primitive_node| {
            let elt = primitive_node.borrow_element();
            let effect = elt.as_filter_effect().unwrap();

            let primitive_values = elt.get_computed_values();

            let primitive_name = format!("{}", primitive_node);

            effect
                .resolve(&primitive_node)
                .map_err(|e| {
                    rsvg_log!(
                        "(filter primitive {} returned an error: {})",
                        primitive_name,
                        e
                    );
                    e
                })
                .and_then(|primitive| {
                    Ok(primitive.to_user_space(
                        user_space_filter.primitive_units,
                        primitive_values,
                        draw_ctx,
                    ))
                })
        })
        .collect::<Result<Vec<UserSpacePrimitive>, FilterError>>();

    let primitives = match res {
        Err(FilterError::CairoError(status)) => {
            // Exit early on Cairo errors
            return Err(RenderingError::from(status));
        }

        Err(_) => {
            // ignore other filter errors and just return an empty surface
            return Ok(SharedImageSurface::empty(
                source_surface.width(),
                source_surface.height(),
                SurfaceType::AlphaOnly,
            )?);
        }

        Ok(r) => r,
    };

    if let Ok(mut filter_ctx) = FilterContext::new(
        &user_space_filter,
        stroke_paint_source,
        fill_paint_source,
        &source_surface,
        transform,
        node_bbox,
    ) {
        for user_space_primitive in primitives {
            let start = Instant::now();

            match render_primitive(&user_space_primitive, &filter_ctx, acquired_nodes, draw_ctx) {
                Ok(output) => {
                    let elapsed = start.elapsed();
                    rsvg_log!(
                        "(rendered filter primitive {} in\n    {} seconds)",
                        user_space_primitive.params.name(),
                        elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) / 1e9
                    );

                    filter_ctx.store_result(FilterResult {
                        name: user_space_primitive.result,
                        output,
                    });
                }

                Err(err) => {
                    rsvg_log!(
                        "(filter primitive {} returned an error: {})",
                        user_space_primitive.params.name(),
                        err
                    );

                    // Exit early on Cairo errors. Continue rendering otherwise.
                    if let FilterError::CairoError(status) = err {
                        return Err(RenderingError::from(status));
                    }
                }
            }
        }

        Ok(filter_ctx.into_output()?)
    } else {
        // Ignore errors that happened when creating the FilterContext
        Ok(SharedImageSurface::empty(
            source_surface.width(),
            source_surface.height(),
            SurfaceType::AlphaOnly,
        )?)
    }
}

#[rustfmt::skip]
fn render_primitive(
    primitive: &UserSpacePrimitive,
    ctx: &FilterContext,
    acquired_nodes: &mut AcquiredNodes<'_>,
    draw_ctx: &mut DrawingCtx,
) -> Result<FilterOutput, FilterError> {
    use PrimitiveParams::*;

    let bounds_builder = primitive.get_bounds(ctx);

    match primitive.params {
        Blend(ref p)             => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        ColorMatrix(ref p)       => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        ComponentTransfer(ref p) => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Composite(ref p)         => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        ConvolveMatrix(ref p)    => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        DiffuseLighting(ref p)   => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        DisplacementMap(ref p)   => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Flood(ref p)             => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        GaussianBlur(ref p)      => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Image(ref p)             => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Merge(ref p)             => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Morphology(ref p)        => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Offset(ref p)            => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        SpecularLighting(ref p)  => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Tile(ref p)              => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
        Turbulence(ref p)        => p.render(bounds_builder, ctx, acquired_nodes, draw_ctx),
    }
}

impl From<ColorInterpolationFilters> for SurfaceType {
    fn from(c: ColorInterpolationFilters) -> Self {
        match c {
            ColorInterpolationFilters::LinearRgb => SurfaceType::LinearRgb,
            _ => SurfaceType::SRgb,
        }
    }
}
