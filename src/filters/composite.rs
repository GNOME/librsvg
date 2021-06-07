use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::shared_surface::Operator as SurfaceOperator;
use crate::xml::Attributes;

use super::bounds::BoundsBuilder;
use super::context::{FilterContext, FilterOutput};
use super::{
    FilterEffect, FilterError, FilterResolveError, Input, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};

/// Enumeration of the possible compositing operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Operator {
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic,
}

enum_default!(Operator, Operator::Over);

/// The `feComposite` filter primitive.
#[derive(Default)]
pub struct FeComposite {
    base: Primitive,
    params: Composite,
}

/// Resolved `feComposite` primitive for rendering.
#[derive(Clone, Default)]
pub struct Composite {
    pub in1: Input,
    pub in2: Input,
    pub operator: Operator,
    pub k1: f64,
    pub k2: f64,
    pub k3: f64,
    pub k4: f64,
    pub color_interpolation_filters: ColorInterpolationFilters,
}

impl SetAttributes for FeComposite {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        let (in1, in2) = self.base.parse_two_inputs(attrs)?;
        self.params.in1 = in1;
        self.params.in2 = in2;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "operator") => self.params.operator = attr.parse(value)?,
                expanded_name!("", "k1") => self.params.k1 = attr.parse(value)?,
                expanded_name!("", "k2") => self.params.k2 = attr.parse(value)?,
                expanded_name!("", "k3") => self.params.k3 = attr.parse(value)?,
                expanded_name!("", "k4") => self.params.k4 = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Composite {
    pub fn render(
        &self,
        bounds_builder: BoundsBuilder,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterOutput, FilterError> {
        let input_1 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in1,
            self.color_interpolation_filters,
        )?;
        let input_2 = ctx.get_input(
            acquired_nodes,
            draw_ctx,
            &self.in2,
            self.color_interpolation_filters,
        )?;
        let bounds: IRect = bounds_builder
            .add_input(&input_1)
            .add_input(&input_2)
            .compute(ctx)
            .clipped
            .into();

        let surface = if self.operator == Operator::Arithmetic {
            input_1.surface().compose_arithmetic(
                input_2.surface(),
                bounds,
                self.k1,
                self.k2,
                self.k3,
                self.k4,
            )?
        } else {
            input_1
                .surface()
                .compose(input_2.surface(), bounds, self.operator.into())?
        };

        Ok(FilterOutput { surface, bounds })
    }
}

impl FilterEffect for FeComposite {
    fn resolve(
        &self,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        node: &Node,
    ) -> Result<ResolvedPrimitive, FilterResolveError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        let mut params = self.params.clone();
        params.color_interpolation_filters = values.color_interpolation_filters();

        Ok(ResolvedPrimitive {
            primitive: self.base.clone(),
            params: PrimitiveParams::Composite(params),
        })
    }
}

impl Parse for Operator {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "over" => Operator::Over,
            "in" => Operator::In,
            "out" => Operator::Out,
            "atop" => Operator::Atop,
            "xor" => Operator::Xor,
            "arithmetic" => Operator::Arithmetic,
        )?)
    }
}

impl From<Operator> for SurfaceOperator {
    #[inline]
    fn from(x: Operator) -> SurfaceOperator {
        use Operator::*;

        match x {
            Over => SurfaceOperator::Over,
            In => SurfaceOperator::In,
            Out => SurfaceOperator::Out,
            Atop => SurfaceOperator::Atop,
            Xor => SurfaceOperator::Xor,

            _ => panic!("can't convert Operator::Arithmetic to a shared_surface::Operator"),
        }
    }
}
