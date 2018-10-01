use std::cell::{Cell, RefCell};

use cairo::{self, ImageSurface};
use cssparser::{CowRcStr, Parser, Token};

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::{NodeError, ValueErrorKind};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::{self, parse, Parse};
use property_bag::PropertyBag;
use surface_utils::{
    iterators::Pixels,
    shared_surface::SharedImageSurface,
    ImageSurfaceDataExt,
    Pixel,
};
use util::clamp;

use super::context::{FilterContext, FilterOutput, FilterResult, IRect};
use super::input::Input;
use super::{Filter, FilterError, PrimitiveWithInput};

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
pub struct Composite {
    base: PrimitiveWithInput,
    in2: RefCell<Option<Input>>,
    operator: Cell<Operator>,
    k1: Cell<f64>,
    k2: Cell<f64>,
    k3: Cell<f64>,
    k4: Cell<f64>,
}

impl Composite {
    /// Constructs a new `Composite` with empty properties.
    #[inline]
    pub fn new() -> Composite {
        Composite {
            base: PrimitiveWithInput::new::<Self>(),
            in2: RefCell::new(None),
            operator: Cell::new(Operator::Over),
            k1: Cell::new(0f64),
            k2: Cell::new(0f64),
            k3: Cell::new(0f64),
            k4: Cell::new(0f64),
        }
    }
}

impl NodeTrait for Composite {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::In2 => {
                    self.in2.replace(Some(Input::parse(Attribute::In2, value)?));
                }
                Attribute::Operator => self.operator.set(parse("operator", value, ())?),
                Attribute::K1 => self.k1.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                Attribute::K2 => self.k2.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                Attribute::K3 => self.k3.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
                Attribute::K4 => self.k4.set(
                    parsers::number(value).map_err(|err| NodeError::attribute_error(attr, err))?,
                ),
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

impl Filter for Composite {
    fn render(
        &self,
        _node: &RsvgNode,
        ctx: &FilterContext,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<FilterResult, FilterError> {
        let input = self.base.get_input(ctx, draw_ctx)?;
        let input_2 = ctx.get_input(draw_ctx, self.in2.borrow().as_ref())?;
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

        let output_surface = if self.operator.get() == Operator::Arithmetic {
            let mut output_surface = ImageSurface::create(
                cairo::Format::ARgb32,
                input.surface().width(),
                input.surface().height(),
            )?;

            let k1 = self.k1.get();
            let k2 = self.k2.get();
            let k3 = self.k3.get();
            let k4 = self.k4.get();

            composite_arithmetic(
                input.surface(),
                input_2.surface(),
                &mut output_surface,
                bounds,
                k1,
                k2,
                k3,
                k4,
            );

            output_surface
        } else {
            let output_surface = input_2.surface().copy_surface(bounds)?;

            let cr = cairo::Context::new(&output_surface);
            cr.rectangle(
                bounds.x0 as f64,
                bounds.y0 as f64,
                (bounds.x1 - bounds.x0) as f64,
                (bounds.y1 - bounds.y0) as f64,
            );
            cr.clip();

            input.surface().set_as_source_surface(&cr, 0f64, 0f64);
            cr.set_operator(self.operator.get().into());
            cr.paint();

            output_surface
        };

        Ok(FilterResult {
            name: self.base.result.borrow().clone(),
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
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _data: Self::Data) -> Result<Self, Self::Err> {
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
