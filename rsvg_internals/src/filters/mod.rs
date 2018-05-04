use std::cell::{Cell, RefCell};

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
