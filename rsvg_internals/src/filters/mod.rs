use std::cell::{Cell, RefCell};
use std::ops::Deref;

use cairo;

use attributes::Attribute;
use drawing_ctx::RsvgDrawingCtx;
use filter_context::RsvgFilterContext;
use error::AttributeError;
use filter_context::{FilterContext, RsvgFilterContext};
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{NodeResult, NodeTrait, RsvgCNodeImpl, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;
use state::State;

mod ffi;
use self::ffi::*;

pub mod offset;

/// A filter primitive interface.
trait Filter: NodeTrait {
    /// Renders this filter primitive.
    ///
    /// If this filter primitive can't be rendered for whatever reason (for instance, a required
    /// property hasn't been provided), return without drawing anything.
    fn render(&self, ctx: *mut RsvgFilterContext);
}

/// The base filter node containing common properties.
struct Primitive {
    // The only purpose of this field is to communicate the render callback back to C code.
    // TODO: get rid of this once all filters have been ported to Rust.
    c_struct: RsvgFilterPrimitive,

    x: Cell<Option<RsvgLength>>,
    y: Cell<Option<RsvgLength>>,
    width: Cell<Option<RsvgLength>>,
    height: Cell<Option<RsvgLength>>,
    result: Cell<Option<String>>,
}

/// An enumeration of possible inputs for a filter primitive.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Input {
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    FilterResult(String),
}

/// The base node for filter primitives which accept input.
struct PrimitiveWithInput {
    base: Primitive,
    in_: RefCell<Option<Input>>,
}

// TODO: remove #[repr(C)] when it's not needed.
#[repr(C)]
pub struct IRect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Primitive {
    /// Constructs a new `Primitive` with empty properties.
    #[inline]
    fn new<T: Filter>() -> Primitive {
        Primitive {
            c_struct: RsvgFilterPrimitive::new::<T>(),

            x: Cell::new(None),
            y: Cell::new(None),
            width: Cell::new(None),
            height: Cell::new(None),
            result: Cell::new(None),
        }
    }

    fn get_bounds(&self, ctx: &FilterContext) -> IRect {
        // TODO: replace with Rust code.
        let mut primitive = RsvgFilterPrimitive::with_props(
            self.x.get(),
            self.y.get(),
            self.width.get(),
            self.height.get(),
        );

        extern "C" {
            fn rsvg_filter_primitive_get_bounds(
                primitive: *mut RsvgFilterPrimitive,
                ctx: *mut RsvgFilterContext, // Actually *const because it doesn't modify it.
            ) -> IRect;
        }

        unsafe {
            rsvg_filter_primitive_get_bounds(
                &mut primitive,
                ctx.get_raw() as *mut RsvgFilterContext,
            )
        }
    }
}

impl NodeTrait for Primitive {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(Some(parse("x", value, LengthDir::Horizontal, None)?)),
                Attribute::Y => self.y.set(Some(parse("y", value, LengthDir::Vertical, None)?)),
                Attribute::Width => self.width.set(Some(parse("width", value, LengthDir::Horizontal, None)?)),
                Attribute::Height => self.height.set(Some(parse("height", value, LengthDir::Vertical, None)?)),
                Attribute::Result => self.result.set(Some(value.to_string())),
                _ => (),
            }
        }

        Ok(())
    }

    #[inline]
    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: &State, _: i32, _: bool) {
        // Nothing; filters are drawn in rsvg-cairo-draw.c.
    }

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        // At least for now we have to return a *const RsvgFilterPrimitive.
        &self.c_struct as *const RsvgFilterPrimitive as *const RsvgCNodeImpl
    }
}

impl Parse for Input {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _data: Self::Data) -> Result<Self, Self::Err> {
        match s {
            "SourceGraphic" => Ok(Input::SourceGraphic),
            "SourceAlpha" => Ok(Input::SourceAlpha),
            "BackgroundImage" => Ok(Input::BackgroundImage),
            "BackgroundAlpha" => Ok(Input::BackgroundAlpha),
            "FillPaint" => Ok(Input::FillPaint),
            "StrokePaint" => Ok(Input::StrokePaint),
            s => Ok(Input::FilterResult(s.to_string())),
        }
    }
}

impl PrimitiveWithInput {
    /// Constructs a new `PrimitiveWithInput` with empty properties.
    #[inline]
    fn new<T: Filter>() -> PrimitiveWithInput {
        PrimitiveWithInput {
            base: Primitive::new(),
            in_: RefCell::new(None),
        }
    }

    /// Returns the input Cairo surface for this filter primitive.
    // TODO: abstract over cairo::ImageSurface, will also allow to handle FillPaint and StrokePaint.
    fn get_input(&self, ctx: &FilterContext) -> Option<cairo::ImageSurface> {
        let in_ = self.in_.borrow();
        if in_.is_none() {
            // No value => use the last result.
            // As per the SVG spec, if the filter primitive is the first in the chain, return the
            // source graphic.
            return Some(ctx.last_result().unwrap_or_else(|| ctx.source_graphic()));
        }

        match *in_.as_ref().unwrap() {
            Input::SourceGraphic => Some(ctx.source_graphic()),
            Input::SourceAlpha => unimplemented!(),
            Input::BackgroundImage => Some(ctx.background_image()),
            Input::BackgroundAlpha => unimplemented!(),

            Input::FillPaint => unimplemented!(),
            Input::StrokePaint => unimplemented!(),

            Input::FilterResult(ref name) => ctx.filter_result(name),
        }
    }
}

impl NodeTrait for PrimitiveWithInput {
    fn set_atts(
        &self,
        node: &RsvgNode,
        handle: *const RsvgHandle,
        pbag: &PropertyBag,
    ) -> NodeResult {
        self.base.set_atts(node, handle, pbag)?;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::In => drop(self.in_.replace(Some(parse("in", value, (), None)?))),
                _ => (),
            }
        }

        Ok(())
    }

    #[inline]
    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: &State, _: i32, _: bool) {
        // Nothing; filters are drawn in rsvg-cairo-draw.c.
    }

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        self.base.get_c_impl()
    }
}

impl Deref for PrimitiveWithInput {
    type Target = Primitive;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
