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
use crate::parsers::{CustomIdent, Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::property_defs::ColorInterpolationFilters;
use crate::surface_utils::shared_surface::{SharedImageSurface, SurfaceType};
use crate::transform::Transform;
use crate::xml::Attributes;

mod bounds;
use self::bounds::BoundsBuilder;

pub mod context;
use self::context::{FilterContext, FilterResult};

mod error;
use self::error::FilterError;

/// A filter primitive interface.
pub trait FilterEffect: SetAttributes + Draw {
    fn resolve(&self, node: &Node) -> Result<(Primitive, PrimitiveParams), FilterError>;
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
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    result: Option<CustomIdent>,
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
                Ok(parse_identifiers!(
                    p,
                    "SourceGraphic" => Input::SourceGraphic,
                    "SourceAlpha" => Input::SourceAlpha,
                    "BackgroundImage" => Input::BackgroundImage,
                    "BackgroundAlpha" => Input::BackgroundAlpha,
                    "FillPaint" => Input::FillPaint,
                    "StrokePaint" => Input::StrokePaint,
                )?)
            })
            .or_else(|_: BasicParseError<'_>| {
                let ident = CustomIdent::parse(parser)?;
                Ok(Input::FilterOutput(ident))
            })
    }
}

impl Primitive {
    fn resolve(
        &self,
        ctx: &FilterContext,
        values: &ComputedValues,
        draw_ctx: &DrawingCtx,
    ) -> Result<ResolvedPrimitive, FilterError> {
        // With ObjectBoundingBox, only fractions and percents are allowed.
        if ctx.primitive_units() == CoordUnits::ObjectBoundingBox {
            check_px_or_percent_units(self.x)?;
            check_px_or_percent_units(self.y)?;
            check_px_or_percent_units(self.width)?;
            check_px_or_percent_units(self.height)?;
        }

        let params = draw_ctx.push_coord_units(ctx.primitive_units());

        let x = self.x.map(|l| l.normalize(values, &params));
        let y = self.y.map(|l| l.normalize(values, &params));
        let width = self.width.map(|l| l.normalize(values, &params));
        let height = self.height.map(|l| l.normalize(values, &params));

        Ok(ResolvedPrimitive {
            x,
            y,
            width,
            height,
            result: self.result.clone(),
        })
    }
}

impl ResolvedPrimitive {
    /// Validates attributes and returns the `BoundsBuilder` for bounds computation.
    #[inline]
    fn get_bounds(&self, ctx: &FilterContext) -> Result<BoundsBuilder, FilterError> {
        Ok(BoundsBuilder::new(
            self.x,
            self.y,
            self.width,
            self.height,
            ctx.paffine(),
        ))
    }
}

fn check_px_or_percent_units<N: Normalize, V: Validate>(
    length: Option<CssLength<N, V>>,
) -> Result<(), FilterError> {
    match length {
        Some(l) if l.unit == LengthUnit::Px || l.unit == LengthUnit::Percent => Ok(()),
        Some(_) => Err(FilterError::InvalidUnits),
        None => Ok(()),
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
    computed_from_node_being_filtered: &ComputedValues,
    source_surface: SharedImageSurface,
    acquired_nodes: &mut AcquiredNodes<'_>,
    draw_ctx: &mut DrawingCtx,
    transform: Transform,
    node_bbox: BoundingBox,
) -> Result<SharedImageSurface, RenderingError> {
    let filter_node = &*filter_node;
    assert!(is_element_of_type!(filter_node, Filter));

    if filter_node.borrow_element().is_in_error() {
        return Ok(source_surface);
    }

    let filter = borrow_element_as!(filter_node, Filter);

    let values = computed_from_node_being_filtered;

    let stroke_paint_source = values
        .stroke()
        .0
        .resolve(acquired_nodes, values.stroke_opacity().0, values.color().0)?
        .to_user_space(&node_bbox, draw_ctx, values);

    let fill_paint_source = values
        .fill()
        .0
        .resolve(acquired_nodes, values.fill_opacity().0, values.color().0)?
        .to_user_space(&node_bbox, draw_ctx, values);

    let resolved_filter = {
        // This is in a temporary scope so we don't leave the coord_units pushed during
        // the execution of all the filter primitives.
        let params = draw_ctx.push_coord_units(filter.get_filter_units());
        filter.resolve(values, &params)
    };

    if let Ok(mut filter_ctx) = FilterContext::new(
        &resolved_filter,
        stroke_paint_source,
        fill_paint_source,
        &source_surface,
        transform,
        node_bbox,
    ) {
        let primitives = filter_node
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
            .filter(|c| c.borrow_element().as_filter_effect().is_some());

        for c in primitives {
            let elt = c.borrow_element();
            let filter = elt.as_filter_effect().unwrap();

            let start = Instant::now();

            if let Err(err) = filter
                .resolve(&c)
                .and_then(|(primitive, params)| {
                    let resolved_primitive = primitive.resolve(
                        &filter_ctx,
                        computed_from_node_being_filtered,
                        draw_ctx,
                    )?;
                    Ok((resolved_primitive, params))
                })
                .and_then(|(resolved_primitive, params)| {
                    render_primitive(
                        &resolved_primitive,
                        &params,
                        &filter_ctx,
                        acquired_nodes,
                        draw_ctx,
                    )
                })
                .and_then(|result| filter_ctx.store_result(result))
            {
                rsvg_log!("(filter primitive {} returned an error: {})", c, err);

                // Exit early on Cairo errors. Continue rendering otherwise.
                if let FilterError::CairoError(status) = err {
                    return Err(RenderingError::from(status));
                }
            }

            let elapsed = start.elapsed();
            rsvg_log!(
                "(rendered filter primitive {} in\n    {} seconds)",
                c,
                elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) / 1e9
            );
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
    primitive: &ResolvedPrimitive,
    params: &PrimitiveParams,
    ctx: &FilterContext,
    acquired_nodes: &mut AcquiredNodes<'_>,
    draw_ctx: &mut DrawingCtx,
) -> Result<FilterResult, FilterError> {
    use PrimitiveParams::*;

    match params {
        Blend(p)             => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        ColorMatrix(p)       => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        ComponentTransfer(p) => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Composite(p)         => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        ConvolveMatrix(p)    => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        DiffuseLighting(p)   => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        DisplacementMap(p)   => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Flood(p)             => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        GaussianBlur(p)      => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Image(p)             => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Merge(p)             => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Morphology(p)        => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Offset(p)            => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        SpecularLighting(p)  => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Tile(p)              => p.render(primitive, ctx, acquired_nodes, draw_ctx),
        Turbulence(p)        => p.render(primitive, ctx, acquired_nodes, draw_ctx),
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
