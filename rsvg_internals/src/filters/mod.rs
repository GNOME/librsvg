use std::cell::{Cell, RefCell};
use std::ops::Deref;

use attributes::Attribute;
use error::AttributeError;
use filter_context::{FilterContext, FilterOutput, FilterResult};
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{NodeResult, NodeTrait, RsvgCNodeImpl, RsvgNode};
use parsers::{parse, Parse};
use property_bag::PropertyBag;

mod ffi;
use self::ffi::*;
pub use self::ffi::{rsvg_filter_render, RsvgFilterPrimitive};

mod error;
use self::error::FilterError;

pub mod offset;

/// A filter primitive interface.
trait Filter: NodeTrait {
    /// Renders this filter primitive.
    ///
    /// If this filter primitive can't be rendered for whatever reason (for instance, a required
    /// property hasn't been provided), an error is returned.
    fn render(&self, node: &RsvgNode, ctx: &FilterContext) -> Result<FilterResult, FilterError>;
}

/// The base filter primitive node containing common properties.
struct Primitive {
    // The purpose of this field is to pass this filter's render function to the C code.
    render_function: RenderFunctionType,

    x: Cell<Option<RsvgLength>>,
    y: Cell<Option<RsvgLength>>,
    width: Cell<Option<RsvgLength>>,
    height: Cell<Option<RsvgLength>>,
    result: RefCell<Option<String>>,
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
    FilterOutput(String),
}

/// The base node for filter primitives which accept input.
struct PrimitiveWithInput {
    base: Primitive,
    in_: RefCell<Option<Input>>,
}

// TODO: remove #[repr(C)] when it's not needed.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Primitive {
    /// Constructs a new `Primitive` with empty properties.
    #[inline]
    fn new<T: Filter>() -> Primitive {
        Primitive {
            render_function: render::<T>,

            x: Cell::new(None),
            y: Cell::new(None),
            width: Cell::new(None),
            height: Cell::new(None),
            result: RefCell::new(None),
        }
    }

    fn get_bounds(&self, ctx: &FilterContext) -> IRect {
        let node = ctx.get_filter_node();
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        ctx.compute_bounds(
            &values,
            self.x.get(),
            self.y.get(),
            self.width.get(),
            self.height.get(),
        )
    }
}

impl NodeTrait for Primitive {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self
                    .x
                    .set(Some(parse("x", value, LengthDir::Horizontal, None)?)),
                Attribute::Y => self
                    .y
                    .set(Some(parse("y", value, LengthDir::Vertical, None)?)),
                Attribute::Width => {
                    self.width
                        .set(Some(parse("width", value, LengthDir::Horizontal, None)?))
                }
                Attribute::Height => {
                    self.height
                        .set(Some(parse("height", value, LengthDir::Vertical, None)?))
                }
                Attribute::Result => *self.result.borrow_mut() = Some(value.to_string()),
                _ => (),
            }
        }

        Ok(())
    }

    #[inline]
    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        // The code that deals with the return value is in ffi.rs.
        self.render_function as *const RenderFunctionType as *const RsvgCNodeImpl
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
            s => Ok(Input::FilterOutput(s.to_string())),
        }
    }
}

impl PrimitiveWithInput {
    /// Constructs a new `PrimitiveWithInput` with empty properties.
    #[inline]
    fn new<T: Filter>() -> PrimitiveWithInput {
        PrimitiveWithInput {
            base: Primitive::new::<T>(),
            in_: RefCell::new(None),
        }
    }

    /// Returns the input Cairo surface for this filter primitive.
    fn get_input(&self, ctx: &FilterContext) -> Option<FilterOutput> {
        let in_ = self.in_.borrow();
        if in_.is_none() {
            // No value => use the last result.
            // As per the SVG spec, if the filter primitive is the first in the chain, return the
            // source graphic.
            return Some(ctx.last_result().cloned().unwrap_or_else(|| FilterOutput {
                surface: ctx.source_graphic().clone(),
                // TODO
                bounds: IRect {
                    x0: 0,
                    y0: 0,
                    x1: 0,
                    y1: 0,
                },
            }));
        }

        match *in_.as_ref().unwrap() {
            Input::SourceGraphic => Some(FilterOutput {
                surface: ctx.source_graphic().clone(),
                // TODO
                bounds: IRect {
                    x0: 0,
                    y0: 0,
                    x1: 0,
                    y1: 0,
                },
            }),
            Input::SourceAlpha => unimplemented!(),
            Input::BackgroundImage => unimplemented!(),
            Input::BackgroundAlpha => unimplemented!(),

            Input::FillPaint => unimplemented!(),
            Input::StrokePaint => unimplemented!(),

            Input::FilterOutput(ref name) => ctx.filter_output(name).cloned(),
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
