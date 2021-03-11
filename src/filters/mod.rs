//! Entry point for the CSS filters infrastructure.

use cssparser::{BasicParseError, Parser};
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::time::Instant;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
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
    fn resolve(&self, node: &Node) -> Result<PrimitiveParams, FilterError>;
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

use composite::FeComposite;
use convolve_matrix::FeConvolveMatrix;
use displacement_map::FeDisplacementMap;
use flood::FeFlood;
use gaussian_blur::FeGaussianBlur;
use image::FeImage;
use lighting::{FeDiffuseLighting, FeSpecularLighting};
use merge::FeMerge;
use morphology::FeMorphology;
use offset::FeOffset;
use tile::FeTile;
use turbulence::FeTurbulence;

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
    Composite(Node),
    ConvolveMatrix(Node),
    DiffuseLighting(Node),
    DisplacementMap(Node),
    Flood(Node),
    GaussianBlur(Node),
    Image(Node),
    Merge(Node),
    Morphology(Node),
    Offset(Node),
    SpecularLighting(Node),
    Tile(Node),
    Turbulence(Node),
}

/// The base filter primitive node containing common properties.
#[derive(Clone)]
struct Primitive {
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    width: Option<ULength<Horizontal>>,
    height: Option<ULength<Vertical>>,
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

impl Default for Input {
    fn default() -> Self {
        Input::Unspecified
    }
}

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
    /// Constructs a new `Primitive` with empty properties.
    #[inline]
    fn new() -> Primitive {
        Primitive {
            x: None,
            y: None,
            width: None,
            height: None,
            result: None,
        }
    }

    /// Validates attributes and returns the `BoundsBuilder` for bounds computation.
    #[inline]
    fn get_bounds(&self, ctx: &FilterContext) -> Result<BoundsBuilder, FilterError> {
        // With ObjectBoundingBox, only fractions and percents are allowed.
        if ctx.primitive_units() == CoordUnits::ObjectBoundingBox {
            check_units(self.x)?;
            check_units(self.y)?;
            check_units(self.width)?;
            check_units(self.height)?;
        }

        let transform = ctx.paffine();
        if !transform.is_invertible() {
            return Err(FilterError::InvalidParameter(
                "transform is not invertible".to_string(),
            ));
        }

        Ok(BoundsBuilder::new(
            self.x,
            self.y,
            self.width,
            self.height,
            transform,
        ))
    }
}

fn check_units<N: Normalize, V: Validate>(
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
    let mut filter_ctx = FilterContext::new(
        &filter,
        computed_from_node_being_filtered,
        source_surface,
        draw_ctx,
        transform,
        node_bbox,
    );

    // If paffine is non-invertible, we won't draw anything. Also bbox combining in bounds
    // computations will panic due to non-invertible martrix.
    if !filter_ctx.paffine().is_invertible() {
        return Ok(filter_ctx.into_output()?);
    }

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
            .and_then(|params| render_primitive(&c, params, &filter_ctx, acquired_nodes, draw_ctx))
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
}

#[rustfmt::skip]
fn render_primitive(
    node: &Node,
    params: PrimitiveParams,
    ctx: &FilterContext,
    acquired_nodes: &mut AcquiredNodes<'_>,
    draw_ctx: &mut DrawingCtx,
) -> Result<FilterResult, FilterError> {
    use PrimitiveParams::*;

    let elt = node.borrow_element();
    let elt = &*elt;

    match (elt, params) {
        (Element::FeBlend(_), Blend(p))                         => p.render(ctx, acquired_nodes, draw_ctx),
        (Element::FeColorMatrix(_), ColorMatrix(p))             => p.render(ctx, acquired_nodes, draw_ctx),
        (Element::FeComponentTransfer(_), ComponentTransfer(p)) => p.render(ctx, acquired_nodes, draw_ctx),
        (Element::FeComposite(ref inner), Composite(node))                 => FeComposite::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeConvolveMatrix(ref inner), ConvolveMatrix(node))       => FeConvolveMatrix::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeDiffuseLighting(ref inner), DiffuseLighting(node))     => FeDiffuseLighting::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeDisplacementMap(ref inner), DisplacementMap(node))     => FeDisplacementMap::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeFlood(ref inner), Flood(node))                         => FeFlood::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeGaussianBlur(ref inner), GaussianBlur(node))           => FeGaussianBlur::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeImage(ref inner), Image(node))                         => FeImage::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeMerge(ref inner), Merge(node))                         => FeMerge::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeMorphology(ref inner), Morphology(node))               => FeMorphology::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeOffset(ref inner), Offset(node))                       => FeOffset::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeSpecularLighting(ref inner), SpecularLighting(node))   => FeSpecularLighting::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeTile(ref inner), Tile(node))                           => FeTile::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (Element::FeTurbulence(ref inner), Turbulence(node))               => FeTurbulence::render(&inner.element_impl, &node, ctx, acquired_nodes, draw_ctx),
        (_, _) => unreachable!(),
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
