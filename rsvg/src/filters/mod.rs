//! Entry point for the CSS filters infrastructure.

use cssparser::{BasicParseError, Parser};
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::rc::Rc;
use std::time::Instant;

use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{set_attribute, ElementTrait};
use crate::error::{InternalRenderingError, ParseError};
use crate::filter::UserSpaceFilter;
use crate::length::*;
use crate::node::Node;
use crate::paint_server::UserSpacePaintSource;
use crate::parse_identifiers;
use crate::parsers::{CustomIdent, Parse, ParseValue};
use crate::properties::ColorInterpolationFilters;
use crate::rsvg_log;
use crate::session::Session;
use crate::surface_utils::{
    shared_surface::{SharedImageSurface, SurfaceType},
    EdgeMode,
};
use crate::transform::Transform;
use crate::xml::Attributes;

mod bounds;
use self::bounds::BoundsBuilder;

pub mod context;
use self::context::{FilterContext, FilterOutput, FilterResult};

mod error;
use self::error::FilterError;
pub use self::error::FilterResolveError;

/// A filter primitive interface.
pub trait FilterEffect: ElementTrait {
    fn resolve(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        node: &Node,
    ) -> Result<Vec<ResolvedPrimitive>, FilterResolveError>;
}

pub mod blend;
pub mod color_matrix;
pub mod component_transfer;
pub mod composite;
pub mod convolve_matrix;
pub mod displacement_map;
pub mod drop_shadow;
pub mod flood;
pub mod gaussian_blur;
pub mod image;
pub mod lighting;
pub mod merge;
pub mod morphology;
pub mod offset;
pub mod tile;
pub mod turbulence;

pub struct FilterSpec {
    pub name: String,
    pub user_space_filter: UserSpaceFilter,
    pub primitives: Vec<UserSpacePrimitive>,
}

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
    pub x: Option<Length<Horizontal>>,
    pub y: Option<Length<Vertical>>,
    pub width: Option<ULength<Horizontal>>,
    pub height: Option<ULength<Vertical>>,
    pub result: Option<CustomIdent>,
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
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub enum Input {
    #[default]
    Unspecified,
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    FilterOutput(CustomIdent),
}

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
    pub fn into_user_space(self, params: &NormalizeParams) -> UserSpacePrimitive {
        let x = self.primitive.x.map(|l| l.to_user(params));
        let y = self.primitive.y.map(|l| l.to_user(params));
        let width = self.primitive.width.map(|l| l.to_user(params));
        let height = self.primitive.height.map(|l| l.to_user(params));

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
        session: &Session,
    ) -> (Input, Input) {
        let mut input_1 = Input::Unspecified;
        let mut input_2 = Input::Unspecified;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => set_attribute(&mut self.x, attr.parse(value), session),
                expanded_name!("", "y") => set_attribute(&mut self.y, attr.parse(value), session),
                expanded_name!("", "width") => {
                    set_attribute(&mut self.width, attr.parse(value), session)
                }
                expanded_name!("", "height") => {
                    set_attribute(&mut self.height, attr.parse(value), session)
                }
                expanded_name!("", "result") => {
                    set_attribute(&mut self.result, attr.parse(value), session)
                }
                expanded_name!("", "in") => set_attribute(&mut input_1, attr.parse(value), session),
                expanded_name!("", "in2") => {
                    set_attribute(&mut input_2, attr.parse(value), session)
                }
                _ => (),
            }
        }

        (input_1, input_2)
    }

    pub fn parse_no_inputs(&mut self, attrs: &Attributes, session: &Session) {
        let (_, _) = self.parse_standard_attributes(attrs, session);
    }

    pub fn parse_one_input(&mut self, attrs: &Attributes, session: &Session) -> Input {
        let (input_1, _) = self.parse_standard_attributes(attrs, session);
        input_1
    }

    pub fn parse_two_inputs(&mut self, attrs: &Attributes, session: &Session) -> (Input, Input) {
        self.parse_standard_attributes(attrs, session)
    }
}

/// Applies a filter and returns the resulting surface.
pub fn render(
    filter: &FilterSpec,
    stroke_paint_source: Rc<UserSpacePaintSource>,
    fill_paint_source: Rc<UserSpacePaintSource>,
    source_surface: SharedImageSurface,
    acquired_nodes: &mut AcquiredNodes<'_>,
    draw_ctx: &mut DrawingCtx,
    transform: Transform,
    node_bbox: BoundingBox,
) -> Result<SharedImageSurface, InternalRenderingError> {
    let session = draw_ctx.session().clone();

    FilterContext::new(
        &filter.user_space_filter,
        stroke_paint_source,
        fill_paint_source,
        &source_surface,
        transform,
        node_bbox,
    )
    .and_then(|mut filter_ctx| {
        // the message has an unclosed parenthesis; we'll close it below.
        rsvg_log!(
            session,
            "(filter \"{}\" with effects_region={:?}",
            filter.name,
            filter_ctx.effects_region()
        );
        for user_space_primitive in &filter.primitives {
            let start = Instant::now();

            match render_primitive(user_space_primitive, &filter_ctx, acquired_nodes, draw_ctx) {
                Ok(output) => {
                    let elapsed = start.elapsed();
                    rsvg_log!(
                        session,
                        "(rendered filter primitive {} in {} seconds)",
                        user_space_primitive.params.name(),
                        elapsed.as_secs() as f64 + f64::from(elapsed.subsec_nanos()) / 1e9
                    );

                    filter_ctx.store_result(FilterResult {
                        name: user_space_primitive.result.clone(),
                        output,
                    });
                }

                Err(err) => {
                    rsvg_log!(
                        session,
                        "(filter primitive {} returned an error: {})",
                        user_space_primitive.params.name(),
                        err
                    );

                    // close the opening parenthesis from the message at the start of this function
                    rsvg_log!(session, ")");

                    // Exit early on Cairo errors. Continue rendering otherwise.
                    if let FilterError::CairoError(status) = err {
                        return Err(FilterError::CairoError(status));
                    }
                }
            }
        }

        // close the opening parenthesis from the message at the start of this function
        rsvg_log!(session, ")");

        Ok(filter_ctx.into_output()?)
    })
    .or_else(|err| match err {
        FilterError::CairoError(status) => {
            // Exit early on Cairo errors
            Err(InternalRenderingError::from(status))
        }

        _ => {
            // ignore other filter errors and just return an empty surface
            Ok(SharedImageSurface::empty(
                source_surface.width(),
                source_surface.height(),
                SurfaceType::AlphaOnly,
            )?)
        }
    })
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

    // Note that feDropShadow is not handled here.  When its FilterElement::resolve() is called,
    // it returns a series of lower-level primitives (flood, blur, offset, etc.) that make up
    // the drop-shadow effect.

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

impl Parse for EdgeMode {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "duplicate" => EdgeMode::Duplicate,
            "wrap" => EdgeMode::Wrap,
            "none" => EdgeMode::None,
        )?)
    }
}
