use cairo::{self, ImageSurface};
use cssparser::{CowRcStr, Parser, Token};
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::drawing_ctx::DrawingCtx;
use crate::error::{AttributeResultExt, ValueErrorKind};
use crate::node::{NodeResult, NodeTrait, RsvgNode};
use crate::parsers::{self, Parse, ParseValue};
use crate::property_bag::PropertyBag;
use crate::rect::IRect;
use crate::surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    ImageSurfaceDataExt,
    Pixel,
};
use crate::util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult};
use super::input::Input;
use super::{FilterEffect, FilterError, PrimitiveWithInput};

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

impl NodeTrait for FeComposite {
    impl_node_as_filter_effect!();

    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.base.set_atts(parent, pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "in2") => self.in2 = Some(Input::parse(attr, value)?),
                expanded_name!(svg "operator") => self.operator = attr.parse(value)?,
                expanded_name!(svg "k1") => self.k1 = parsers::number(value).attribute(attr)?,
                expanded_name!(svg "k2") => self.k2 = parsers::number(value).attribute(attr)?,
                expanded_name!(svg "k3") => self.k3 = parsers::number(value).attribute(attr)?,
                expanded_name!(svg "k4") => self.k4 = parsers::number(value).attribute(attr)?,
                _ => (),
            }
        }

        Ok(())
    }
}

/// Performs the arithmetic composite operation. Public for benchmarking.
#[inline]
pub fn composite_arithmetic(
    input_surface: &SharedImageSurface,
    input_2_surface: &SharedImageSurface,
    output_surface: &mut cairo::ImageSurface,
    bounds: IRect,
    k1: f64,
    k2: f64,
    k3: f64,
    k4: f64,
) {
    let output_stride = output_surface.get_stride() as usize;
    {
        let mut output_data = output_surface.get_data().unwrap();

        for (x, y, pixel, pixel_2) in Pixels::new(input_surface, bounds)
            .map(|(x, y, p)| (x, y, p, input_2_surface.get_pixel(x, y)))
        {
            let i1a = f64::from(pixel.a) / 255f64;
            let i2a = f64::from(pixel_2.a) / 255f64;
            let oa = k1 * i1a * i2a + k2 * i1a + k3 * i2a + k4;
            let oa = clamp(oa, 0f64, 1f64);

            // Contents of image surfaces are transparent by default, so if the resulting pixel is
            // transparent there's no need to do anything.
            if oa > 0f64 {
                let compute = |i1, i2| {
                    let i1 = f64::from(i1) / 255f64;
                    let i2 = f64::from(i2) / 255f64;

                    let o = k1 * i1 * i2 + k2 * i1 + k3 * i2 + k4;
                    let o = clamp(o, 0f64, oa);

                    ((o * 255f64) + 0.5) as u8
                };

                let output_pixel = Pixel {
                    r: compute(pixel.r, pixel_2.r),
                    g: compute(pixel.g, pixel_2.g),
                    b: compute(pixel.b, pixel_2.b),
                    a: ((oa * 255f64) + 0.5) as u8,
                };
                output_data.set_pixel(output_stride, output_pixel, x, y);
            }
        }
    }
}

impl FilterEffect for FeComposite {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let input_2 = ctx.get_input(draw_ctx, self.in2.as_ref())?;
        let bounds = self
            .base
            .get_bounds(ctx)
            .add_input(&input)
            .add_input(&input_2)
            .into_irect(draw_ctx);

        // If we're combining two alpha-only surfaces, the result is alpha-only. Otherwise the
        // result is whatever the non-alpha-only type we're working on (which can be either sRGB or
        // linear sRGB depending on color-interpolation-filters).
        let surface_type = if input.surface().is_alpha_only() {
            input_2.surface().surface_type()
        } else {
            if !input_2.surface().is_alpha_only() {
                // All surface types should match (this is enforced by get_input()).
                assert_eq!(
                    input_2.surface().surface_type(),
                    input.surface().surface_type()
                );
            }

            input.surface().surface_type()
        };

        let output_surface = if self.operator == Operator::Arithmetic {
            let mut output_surface = ImageSurface::create(
                cairo::Format::ARgb32,
                input.surface().width(),
                input.surface().height(),
            )?;

            composite_arithmetic(
                input.surface(),
                input_2.surface(),
                &mut output_surface,
                bounds,
                self.k1,
                self.k2,
                self.k3,
                self.k4,
            );

            output_surface
        } else {
            let output_surface = input_2.surface().copy_surface(bounds)?;

            let cr = cairo::Context::new(&output_surface);
            let r = cairo::Rectangle::from(bounds);
            cr.rectangle(r.x, r.y, r.width, r.height);
            cr.clip();

            input.surface().set_as_source_surface(&cr, 0f64, 0f64);
            cr.set_operator(self.operator.into());
            cr.paint();

            output_surface
        };

        Ok(FilterResult {
            name: self.base.result.clone(),
            output: FilterOutput {
                surface: SharedImageSurface::new(output_surface, surface_type)?,
                bounds,
            },
        })
    }

    #[inline]
    fn is_affected_by_color_interpolation_filters(&self) -> bool {
        true
    }
}

impl Parse for Operator {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Self, Self::Err> {
        let loc = parser.current_source_location();

        parser
            .expect_ident()
            .and_then(|cow| match cow.as_ref() {
                "over" => Ok(Operator::Over),
                "in" => Ok(Operator::In),
                "out" => Ok(Operator::Out),
                "atop" => Ok(Operator::Atop),
                "xor" => Ok(Operator::Xor),
                "arithmetic" => Ok(Operator::Arithmetic),
                _ => Err(
                    loc.new_basic_unexpected_token_error(Token::Ident(CowRcStr::from(
                        cow.as_ref().to_string(),
                    ))),
                ),
            })
            .map_err(|_| ValueErrorKind::Value("invalid operator value".to_string()))
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
