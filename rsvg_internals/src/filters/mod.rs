use std::cell::{Cell, RefCell};
use std::ops::Deref;

use attributes::Attribute;
use handle::RsvgHandle;
use length::{LengthDir, RsvgLength};
use node::{NodeResult, NodeTrait, RsvgCNodeImpl, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;

pub mod context;
use self::context::{FilterContext, FilterOutput, FilterResult, IRect};

mod error;
use self::error::FilterError;

mod ffi;
use self::ffi::*;
pub use self::ffi::{rsvg_filter_render, RsvgFilterPrimitive};

pub mod input;
use self::input::Input;

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
        ctx.get_input(self.in_.borrow().as_ref())
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
