use std::cmp::min;

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::node::{NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::number_list::{NumberList, NumberListLength};
use crate::parsers::{Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::surface_utils::{
    iterators::Pixels, shared_surface::SharedImageSurface, ImageSurfaceDataExt, Pixel,
};
use crate::util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::{FilterEffect, FilterError, PrimitiveWithInput};

/// The `feComponentTransfer` filter primitive.
pub struct FeComponentTransfer {
    base: PrimitiveWithInput,
}

impl Default for FeComponentTransfer {
    /// Constructs a new `ComponentTransfer` with empty properties.
    #[inline]
    fn default() -> FeComponentTransfer {
        FeComponentTransfer {
            base: PrimitiveWithInput::new::<Self>(),
        }
    }
}

impl NodeTrait for FeComponentTransfer {
    impl_node_as_filter_effect!();

    #[inline]
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)
    }
}

/// Pixel components that can be influenced by `feComponentTransfer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    R,
    G,
    B,
    A,
}

/// Component transfer function types.
enum FunctionType {
    Identity,
    Table,
    Discrete,
    Linear,
    Gamma,
}

impl Parse for FunctionType {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>> {
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
struct FunctionParameters<'a> {
    table_values: &'a Vec<f64>,
    slope: f64,
    intercept: f64,
    amplitude: f64,
    exponent: f64,
    offset: f64,
}

/// The compute function type.
type Function = fn(&FunctionParameters<'_>, f64) -> f64;

/// The identity component transfer function.
fn identity(_: &FunctionParameters<'_>, value: f64) -> f64 {
    value
}

/// The table component transfer function.
fn table(params: &FunctionParameters<'_>, value: f64) -> f64 {
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
fn discrete(params: &FunctionParameters<'_>, value: f64) -> f64 {
    let n = params.table_values.len();
    let k = (value * (n as f64)).floor() as usize;

    params.table_values[min(k, n - 1)]
}

/// The linear component transfer function.
fn linear(params: &FunctionParameters<'_>, value: f64) -> f64 {
    params.slope * value + params.intercept
}

/// The gamma component transfer function.
fn gamma(params: &FunctionParameters<'_>, value: f64) -> f64 {
    params.amplitude * value.powf(params.exponent) + params.offset
}

trait FeComponentTransferFunc {
    /// Returns the component transfer function.
    fn function(&self) -> Function;

    /// Returns the component transfer function parameters.
    fn function_parameters(&self) -> FunctionParameters<'_>;

    /// Returns the channel.
    fn channel(&self) -> Channel;
}

macro_rules! func_x {
    ($func_name:ident, $channel:expr) => {
        pub struct $func_name {
            channel: Channel,
            function_type: FunctionType,
            table_values: Vec<f64>,
            slope: f64,
            intercept: f64,
            amplitude: f64,
            exponent: f64,
            offset: f64,
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
            fn function_parameters(&self) -> FunctionParameters<'_> {
                FunctionParameters {
                    table_values: &self.table_values,
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

        impl NodeTrait for $func_name {
            #[inline]
            fn set_atts(
                &mut self,
                _parent: Option<&RsvgNode>,
                pbag: &PropertyBag<'_>,
            ) -> NodeResult {
                for (attr, value) in pbag.iter() {
                    match attr.expanded() {
                        expanded_name!("", "type") => {
                            self.function_type = attr.parse(value)?
                        }
                        expanded_name!("", "tableValues") => {
                            let NumberList(v) =
                                NumberList::parse_str(value, NumberListLength::Unbounded)
                                .attribute(attr)?;
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
    ($func_node:ident, $func_type:ty, $func_data:ident, $func_default:ident) => {
        match $func_node {
            Some(ref f) => {
                $func_data = f.borrow();
                $func_data.get_impl::<$func_type>()
            }
            _ => &$func_default,
        };
    };
}

impl FilterEffect for FeComponentTransfer {
    fn render(
        &self,
        node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .into_irect(draw_ctx);

        // Create the output surface.
        let mut output_surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        // Get a node for every pixel component.
        fn get_node<F>(node: &RsvgNode, node_type: NodeType, channel: Channel) -> Option<RsvgNode>
        where
            F: FeComponentTransferFunc + NodeTrait,
        {
            node.children()
                .rev()
                .filter(|c| c.borrow().get_type() == node_type)
                .find(|c| c.borrow().get_impl::<F>().channel() == channel)
        };

        let func_r_node = get_node::<FeFuncR>(node, NodeType::FeFuncR, Channel::R);
        let func_g_node = get_node::<FeFuncG>(node, NodeType::FeFuncG, Channel::G);
        let func_b_node = get_node::<FeFuncB>(node, NodeType::FeFuncB, Channel::B);
        let func_a_node = get_node::<FeFuncA>(node, NodeType::FeFuncA, Channel::A);

        for node in [&func_r_node, &func_g_node, &func_b_node, &func_a_node]
            .iter()
            .filter_map(|x| x.as_ref())
        {
            if node.borrow().is_in_error() {
                return Err(FilterError::ChildNodeInError);
            }
        }

        // These are the default funcs that perform an identity transformation.
        let func_r_default = FeFuncR::default();
        let func_g_default = FeFuncG::default();
        let func_b_default = FeFuncB::default();
        let func_a_default = FeFuncA::default();

        // We need to tell the borrow checker that these live long enough
        let func_r_data;
        let func_g_data;
        let func_b_data;
        let func_a_data;

        let func_r = func_or_default!(func_r_node, FeFuncR, func_r_data, func_r_default);
        let func_g = func_or_default!(func_g_node, FeFuncG, func_g_data, func_g_default);
        let func_b = func_or_default!(func_b_node, FeFuncB, func_b_data, func_b_default);
        let func_a = func_or_default!(func_a_node, FeFuncA, func_a_data, func_a_default);

        #[inline]
        fn compute_func<'a, F>(func: &'a F) -> impl Fn(u8, f64, f64) -> u8 + 'a
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

        let compute_r = compute_func::<FeFuncR>(&func_r);
        let compute_g = compute_func::<FeFuncG>(&func_g);
        let compute_b = compute_func::<FeFuncB>(&func_b);

        // Alpha gets special handling since everything else depends on it.
        let compute_a = func_a.function();
        let params_a = func_a.function_parameters();
        let compute_a = |alpha| compute_a(&params_a, alpha);

        // Do the actual processing.
        let output_stride = output_surface.get_stride() as usize;
        {
            let mut output_data = output_surface.get_data().unwrap();

            for (x, y, pixel) in Pixels::new(input.surface(), bounds) {
                let alpha = f64::from(pixel.a) / 255f64;
                let new_alpha = compute_a(alpha);

                let output_pixel = Pixel {
                    r: compute_r(pixel.r, alpha, new_alpha),
                    g: compute_g(pixel.g, alpha, new_alpha),
                    b: compute_b(pixel.b, alpha, new_alpha),
                    a: ((new_alpha * 255f64) + 0.5) as u8,
                };

                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, input.surface().surface_type())?,
                bounds,
            },
        })
    }

    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}
