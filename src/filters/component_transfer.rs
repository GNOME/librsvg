use std::cmp::min;

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::*;
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::parsers::{NumberList, Parse, ParseValue};
use crate::properties::ColorInterpolationFilters;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::Pixels, shared_surface::ExclusiveImageSurface, ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;
use crate::xml::Attributes;

use super::bounds::BoundsBuilder;
use super::context::{FilterContext, FilterOutput};
use super::{
    FilterEffect, FilterError, FilterResolveError, Input, Primitive, PrimitiveParams,
    ResolvedPrimitive,
};

/// The `feComponentTransfer` filter primitive.
#[derive(Default)]
pub struct FeComponentTransfer {
    base: Primitive,
    params: ComponentTransfer,
}

/// Resolved `feComponentTransfer` primitive for rendering.
#[derive(Clone, Default)]
pub struct ComponentTransfer {
    pub in1: Input,
    pub functions: Functions,
    pub color_interpolation_filters: ColorInterpolationFilters,
}

impl SetAttributes for FeComponentTransfer {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.params.in1 = self.base.parse_one_input(attrs)?;
        Ok(())
    }
}

/// Pixel components that can be influenced by `feComponentTransfer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    R,
    G,
    B,
    A,
}

/// Component transfer function types.
#[derive(Clone, Debug, PartialEq)]
pub enum FunctionType {
    Identity,
    Table,
    Discrete,
    Linear,
    Gamma,
}

impl Parse for FunctionType {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "identity" => FunctionType::Identity,
            "table" => FunctionType::Table,
            "discrete" => FunctionType::Discrete,
            "linear" => FunctionType::Linear,
            "gamma" => FunctionType::Gamma,
        )?)
    }
}

/// The compute function parameters.
struct FunctionParameters {
    table_values: Vec<f64>,
    slope: f64,
    intercept: f64,
    amplitude: f64,
    exponent: f64,
    offset: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Functions {
    pub r: FeFuncR,
    pub g: FeFuncG,
    pub b: FeFuncB,
    pub a: FeFuncA,
}

/// The compute function type.
type Function = fn(&FunctionParameters, f64) -> f64;

/// The identity component transfer function.
fn identity(_: &FunctionParameters, value: f64) -> f64 {
    value
}

/// The table component transfer function.
fn table(params: &FunctionParameters, value: f64) -> f64 {
    let n = params.table_values.len() - 1;
    let k = (value * (n as f64)).floor() as usize;

    let k = min(k, n); // Just in case.

    if k == n {
        return params.table_values[k];
    }

    let vk = params.table_values[k];
    let vk1 = params.table_values[k + 1];
    let k = k as f64;
    let n = n as f64;

    vk + (value - k / n) * n * (vk1 - vk)
}

/// The discrete component transfer function.
fn discrete(params: &FunctionParameters, value: f64) -> f64 {
    let n = params.table_values.len();
    let k = (value * (n as f64)).floor() as usize;

    params.table_values[min(k, n - 1)]
}

/// The linear component transfer function.
fn linear(params: &FunctionParameters, value: f64) -> f64 {
    params.slope * value + params.intercept
}

/// The gamma component transfer function.
fn gamma(params: &FunctionParameters, value: f64) -> f64 {
    params.amplitude * value.powf(params.exponent) + params.offset
}

trait FeComponentTransferFunc {
    /// Returns the component transfer function.
    fn function(&self) -> Function;

    /// Returns the component transfer function parameters.
    fn function_parameters(&self) -> FunctionParameters;

    /// Returns the channel.
    fn channel(&self) -> Channel;
}

macro_rules! func_x {
    ($func_name:ident, $channel:expr) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct $func_name {
            pub channel: Channel,
            pub function_type: FunctionType,
            pub table_values: Vec<f64>,
            pub slope: f64,
            pub intercept: f64,
            pub amplitude: f64,
            pub exponent: f64,
            pub offset: f64,
        }

        impl Default for $func_name {
            #[inline]
            fn default() -> Self {
                Self {
                    channel: $channel,
                    function_type: FunctionType::Identity,
                    table_values: Vec::new(),
                    slope: 1.0,
                    intercept: 0.0,
                    amplitude: 1.0,
                    exponent: 1.0,
                    offset: 0.0,
                }
            }
        }

        impl FeComponentTransferFunc for $func_name {
            #[inline]
            fn function_parameters(&self) -> FunctionParameters {
                FunctionParameters {
                    table_values: self.table_values.clone(),
                    slope: self.slope,
                    intercept: self.intercept,
                    amplitude: self.amplitude,
                    exponent: self.exponent,
                    offset: self.offset,
                }
            }

            #[inline]
            fn function(&self) -> Function {
                match self.function_type {
                    FunctionType::Identity => identity,
                    FunctionType::Table => table,
                    FunctionType::Discrete => discrete,
                    FunctionType::Linear => linear,
                    FunctionType::Gamma => gamma,
                }
            }

            #[inline]
            fn channel(&self) -> Channel {
                self.channel
            }
        }

        impl SetAttributes for $func_name {
            #[inline]
            fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
                for (attr, value) in attrs.iter() {
                    match attr.expanded() {
                        expanded_name!("", "type") => self.function_type = attr.parse(value)?,
                        expanded_name!("", "tableValues") => {
                            // #691: Limit list to 256 to mitigate malicious SVGs
                            let NumberList::<0, 256>(v) = attr.parse(value)?;
                            self.table_values = v;
                        }
                        expanded_name!("", "slope") => self.slope = attr.parse(value)?,
                        expanded_name!("", "intercept") => self.intercept = attr.parse(value)?,
                        expanded_name!("", "amplitude") => self.amplitude = attr.parse(value)?,
                        expanded_name!("", "exponent") => self.exponent = attr.parse(value)?,
                        expanded_name!("", "offset") => self.offset = attr.parse(value)?,

                        _ => (),
                    }
                }

                // The table function type with empty table_values is considered
                // an identity function.
                match self.function_type {
                    FunctionType::Table | FunctionType::Discrete => {
                        if self.table_values.is_empty() {
                            self.function_type = FunctionType::Identity;
                        }
                    }
                    _ => (),
                }

                Ok(())
            }
        }

        impl Draw for $func_name {}
    };
}

// The `<feFuncR>` element
func_x!(FeFuncR, Channel::R);

// The `<feFuncG>` element
func_x!(FeFuncG, Channel::G);

// The `<feFuncB>` element
func_x!(FeFuncB, Channel::B);

// The `<feFuncA>` element
func_x!(FeFuncA, Channel::A);

macro_rules! func_or_default {
    ($func_node:ident, $func_type:ident) => {
        match $func_node {
            Some(ref f) => match *f.borrow_element() {
                Element::$func_type(ref e) => e.element_impl.clone(),
                _ => unreachable!(),
            },
            _ => $func_type::default(),
        }
    };
}

macro_rules! get_func_x_node {
    ($func_node:ident, $func_type:ident, $channel:expr) => {
        $func_node
            .children()
            .rev()
            .filter(|c| c.is_element())
            .find(|c| match *c.borrow_element() {
                Element::$func_type(ref f) => f.channel() == $channel,
                _ => false,
            })
    };
}

impl ComponentTransfer {
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
        let bounds: IRect = bounds_builder
            .add_input(&input_1)
            .compute(ctx)
            .clipped
            .into();

        // Create the output surface.
        let mut surface = ExclusiveImageSurface::new(
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
            input_1.surface().surface_type(),
        )?;

        #[inline]
        fn compute_func<F>(func: &F) -> impl Fn(u8, f64, f64) -> u8
        where
            F: FeComponentTransferFunc,
        {
            let compute = func.function();
            let params = func.function_parameters();

            move |value, alpha, new_alpha| {
                let value = f64::from(value) / 255f64;

                let unpremultiplied = if alpha == 0f64 { 0f64 } else { value / alpha };

                let new_value = compute(&params, unpremultiplied);
                let new_value = clamp(new_value, 0f64, 1f64);

                ((new_value * new_alpha * 255f64) + 0.5) as u8
            }
        }

        let compute_r = compute_func::<FeFuncR>(&self.functions.r);
        let compute_g = compute_func::<FeFuncG>(&self.functions.g);
        let compute_b = compute_func::<FeFuncB>(&self.functions.b);

        // Alpha gets special handling since everything else depends on it.
        let compute_a = self.functions.a.function();
        let params_a = self.functions.a.function_parameters();
        let compute_a = |alpha| compute_a(&params_a, alpha);

        // Do the actual processing.
        surface.modify(&mut |data, stride| {
            for (x, y, pixel) in Pixels::within(input_1.surface(), bounds) {
                let alpha = f64::from(pixel.a) / 255f64;
                let new_alpha = compute_a(alpha);

                let output_pixel = Pixel {
                    r: compute_r(pixel.r, alpha, new_alpha),
                    g: compute_g(pixel.g, alpha, new_alpha),
                    b: compute_b(pixel.b, alpha, new_alpha),
                    a: ((new_alpha * 255f64) + 0.5) as u8,
                };

                data.set_pixel(stride, output_pixel, x, y);
            }
        });

        Ok(FilterOutput {
            surface: surface.share()?,
            bounds,
        })
    }
}

impl FilterEffect for FeComponentTransfer {
    fn resolve(
        &self,
        _acquired_nodes: &mut AcquiredNodes<'_>,
        node: &Node,
    ) -> Result<ResolvedPrimitive, FilterResolveError> {
        let cascaded = CascadedValues::new_from_node(node);
        let values = cascaded.get();

        let mut params = self.params.clone();
        params.functions = get_functions(node)?;
        params.color_interpolation_filters = values.color_interpolation_filters();

        Ok(ResolvedPrimitive {
            primitive: self.base.clone(),
            params: PrimitiveParams::ComponentTransfer(params),
        })
    }
}

/// Takes a feComponentTransfer and walks its children to produce the feFuncX arguments.
fn get_functions(node: &Node) -> Result<Functions, FilterResolveError> {
    let func_r_node = get_func_x_node!(node, FeFuncR, Channel::R);
    let func_g_node = get_func_x_node!(node, FeFuncG, Channel::G);
    let func_b_node = get_func_x_node!(node, FeFuncB, Channel::B);
    let func_a_node = get_func_x_node!(node, FeFuncA, Channel::A);

    for node in [&func_r_node, &func_g_node, &func_b_node, &func_a_node]
        .iter()
        .filter_map(|x| x.as_ref())
    {
        if node.borrow_element().is_in_error() {
            return Err(FilterResolveError::ChildNodeInError);
        }
    }

    let r = func_or_default!(func_r_node, FeFuncR);
    let g = func_or_default!(func_g_node, FeFuncG);
    let b = func_or_default!(func_b_node, FeFuncB);
    let a = func_or_default!(func_a_node, FeFuncA);

    Ok(Functions { r, g, b, a })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn extracts_functions() {
        let document = Document::load_from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <filter id="filter">
    <feComponentTransfer id="component_transfer">
      <!-- no feFuncR so it should get the defaults -->

      <feFuncG type="table" tableValues="0.0 1.0 2.0"/>

      <feFuncB type="table"/>
      <!-- duplicate this to test that last-one-wins -->
      <feFuncB type="discrete" tableValues="0.0, 1.0" slope="1.0" intercept="2.0" amplitude="3.0" exponent="4.0" offset="5.0"/>

      <!-- no feFuncA so it should get the defaults -->
    </feComponentTransfer>
  </filter>
</svg>
"#
        );

        let component_transfer = document.lookup_internal_node("component_transfer").unwrap();
        let functions = get_functions(&component_transfer).unwrap();

        assert_eq!(
            functions,
            Functions {
                r: FeFuncR::default(),

                g: FeFuncG {
                    function_type: FunctionType::Table,
                    table_values: vec![0.0, 1.0, 2.0],
                    ..FeFuncG::default()
                },

                b: FeFuncB {
                    function_type: FunctionType::Discrete,
                    table_values: vec![0.0, 1.0],
                    slope: 1.0,
                    intercept: 2.0,
                    amplitude: 3.0,
                    exponent: 4.0,
                    offset: 5.0,
                    ..FeFuncB::default()
                },

                a: FeFuncA::default(),
            }
        );
    }
}
