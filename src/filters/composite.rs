use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::attributes::Attributes;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{ElementResult, SetAttributes};
use crate::error::*;
use crate::node::Node;
use crate::parsers::{Parse, ParseValue};

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, Input, PrimitiveWithInput};

/// Enumeration of the possible compositing operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum Operator {
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic,
}

/// The `feComposite` filter primitive.
pub struct FeComposite {
    base: PrimitiveWithInput,
    in2: Option<Input>,
    operator: Operator,
    k1: f64,
    k2: f64,
    k3: f64,
    k4: f64,
}

impl Default for FeComposite {
    /// Constructs a new `Composite` with empty properties.
    #[inline]
    fn default() -> FeComposite {
        FeComposite {
            base: PrimitiveWithInput::new::<Self>(),
            in2: None,
            operator: Operator::Over,
            k1: 0.0,
            k2: 0.0,
            k3: 0.0,
            k4: 0.0,
        }
    }
}

impl SetAttributes for FeComposite {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.base.set_attributes(attrs)?;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "in2") => self.in2 = Some(attr.parse(value)?),
                expanded_name!("", "operator") => self.operator = attr.parse(value)?,
                expanded_name!("", "k1") => self.k1 = attr.parse(value)?,
                expanded_name!("", "k2") => self.k2 = attr.parse(value)?,
                expanded_name!("", "k3") => self.k3 = attr.parse(value)?,
                expanded_name!("", "k4") => self.k4 = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl FilterEffect for FeComposite {
    fn render(
        &self,
        node: &Node,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, acquired_nodes, draw_ctx)?;
        let input_2 = ctx.get_input(acquired_nodes, draw_ctx, self.in2.as_ref())?;
        let bounds = self
            .base
            .get_bounds(ctx, node.parent().as_ref())?
            .add_input(&input)
            .add_input(&input_2)
            .into_irect(draw_ctx);

        let surface = if self.operator == Operator::Arithmetic {
            input.surface().compose_arithmetic(
                input_2.surface(),
                bounds,
                self.k1,
                self.k2,
                self.k3,
                self.k4,
            )?
        } else {
            input.surface().compose(
                input_2.surface(),
                bounds,
                cairo::Operator::from(self.operator),
            )?
        };

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput { surface, bounds },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
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

impl From<Operator> for cairo::Operator {
    #[inline]
    fn from(x: Operator) -> Self {
        match x {
            Operator::Over => cairo::Operator::Over,
            Operator::In => cairo::Operator::In,
            Operator::Out => cairo::Operator::Out,
            Operator::Atop => cairo::Operator::Atop,
            Operator::Xor => cairo::Operator::Xor,
            _ => panic!("can't convert Operator::Arithmetic to a cairo::Operator"),
        }
    }
}
