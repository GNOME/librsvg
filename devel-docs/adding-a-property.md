# Adding a new CSS property to librsvg

This document is a little tour on how to add support for a CSS property to librsvg.  We
will implement the [`mask-type`
property](https://www.w3.org/TR/css-masking-1/#the-mask-type) from the **CSS Masking
Module Level 1** specification.

## What is `mask-type`?

[The spec says about `mask-type`](https://www.w3.org/TR/css-masking-1/#the-mask-type):

> The mask-type property defines whether the content of the mask element is treated as as
> luminance mask or alpha mask, as described in Calculating mask values.

A **luminance mask** takes the RGB values of each pixel, converts them to a single luminance
value, and uses that as a mask.

An **alpha mask** just takes the alpha value of each pixel and uses it as a mask.

The only mask type that SVG1.1 supported was luminance masks; there
wasn't even a `mask-type` property back then.  The SVG2 spec removed
descriptions of masking, and offloaded them to the [CSS Masking Module
Level 1](https://www.w3.org/TR/css-masking-1/) specification, which it
adds the `mask-type` property and others as well.

Let's start by figuring out how to read the spec.

## What the specification says

The specification for `mask-type` is in https://www.w3.org/TR/css-masking-1/#the-mask-type 

In the specs, most of the descriptions for properties start with a table that summarizes
the property.  For example, if you visit that link, you will find a table that starts with
these items:

* **Name:**           `mask-type`
* **Value:**          `luminance | alpha`
* **Initial:**        `luminance`
* **Applies to:**     mask elements
* **Inherited:**      no
* **Computed value:** as specified

Let's go through each of these:

**Name:** We have the name of the property (`mask-type`).  Properties are case-insensitive, and
librsvg already has machinery to handle that.

**Value:** The possible values for the property can be `luminance` or `alpha`.  In the spec's web page,
even the little `|` between those two values is a hyperlink; clicking it will take you to
the specification for CSS Values and Units, where it describes the grammar that the CSS
specs use to describe their values.  Here you just need to know that `|` means
that exactly one of the two alternatives must occur.

As you may imagine, librsvg already parses a lot of similar properties that are just
symbolic values.  For example, the `stroke-linecap` property can have values `butt | round
| square`.  We'll see how to write a parser for this kind of property with a minimal amount of code.

**Initial:** Then there is the initial or default value, which is `luminance`.  This means
that if the `mask-type` property is not specified on an element, it takes `luminance` as
its default.  This is a sensible choice, since an SVG1.1 file that is processed by SVG2
software should retain the same semantics.  It also means that if there is a parse error,
for example if you typed `ahlpha`, the property will silently revert back to the default
`luminance` value.

**Applies to:** Librsvg doesn't pay much attention to "applies to" — it just carries
property values for all elements, and the elements that don't handle a property just
ignore it.

**Inherited:** This property is not inherited, which means that by default, its value does
not cascade.  So if you have this:

```xml
<mask style="mask-type: alpha;">
  <other>
    <elements>
      <here/>
    </elements>
  </other>
</mask>
```

Then the `other`, `elements`, `here` will not inherit the `mask-type` value from their ancestor.

**Computed value:** Finally, the computed value is "as specified", which means that
librsvg does not need to modify it in any way when resolving the CSS cascade.  Other
properties, like `width: 1em;` may need to be resolved against the `font-size` to obtain
the computed value.

The W3C specifications can get pretty verbose and it takes some practice to read them, but
fortunately this property is short and sweet.

Let's go on.

## How librsvg represents properties

Each property has a Rust type that can hold its values.  Remember the part of the masking
spec from above, that says the `mask-type` property can have values `luminance` or
`alpha`, and the initial/default is `luminance`?  This translates easily to Rust types:

```rust
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MaskType {
    Luminance,
    Alpha,
}

impl Default for MaskType {
    fn default() -> MaskType {
        MaskType::Luminance
    }
}
```

Additionally, we need to be able to say that the property does not inherit by default, and
that its computed value is the same as the specified value (e.g. we can just copy the
original value without changing it).  Librsvg defines a `Property` trait for those actions:

```rust
pub trait Property {
    fn inherits_automatically() -> bool;

    fn compute(&self, _: &ComputedValues) -> Self;
}
```

For the `mask-type` property, we want `inherits_automatically` to return `false`, and
`compute` to return the value unchanged.  So, like this:

```rust
impl Property for MaskType {
    fn inherits_automatically() -> bool {
        false
    }
    
    fn compute(&self, _: &ComputedValues) -> Self {
        self.clone()
    }
}
```

Ignore the `ComputedValues` argument for now — it is how librsvg represents an element's
complete set of property values.

As you can imagine, there are a lot of properties like `mask-type`, whose values are just
symbolic names that map well to a data-less enum.  For all of them, it would be a lot of
repetitive code to define their default value, return whether they inherit or not, and
clone them for the computed value.  Additionally, we have not even written the parser for
this property's values yet.

Fortunately, librsvg has a `make_property!` macro that lets you
do this instead:

```rust
make_property!(
    /// `mask-type` property.                                          // (1)
    ///
    /// https://www.w3.org/TR/css-masking-1/#the-mask-type
    MaskType,                                                          // (2)
    default: Luminance,                                                // (3)
    inherits_automatically: false,                                     // (4)

    identifiers:                                                       // (5)
    "luminance" => Luminance,
    "alpha" => Alpha,
);
```

* (1) is a documentation comment for the `MaskType` enum being defined.

* (2) is `MaskType`, the name we will use for the `mask-type` property.

* (3) indicates the "initial value", or default, for the property.

* (4) ... whether the spec says the property should inherit or not.

* (5) Finally, `identifiers:` is what makes the `make_property!` macro know that it should
  generate a parser for the symbolic names `luminance` and `alpha`, and that they should
  correspond to the values `MaskType::Luminance` and `MaskType::Alpha`, respectively.
  
This saves a lot of typing!  Also, it makes it easier to gradually change the way
properties are represented, as librsvg evolves.

## Properties that use the same data type

Consider the `stroke` and `fill` properties; both store a
[`<paint>`](https://www.w3.org/TR/SVG2/painting.html#SpecifyingPaint) value, which librsvg
represents with a type called `PaintServer`.  The `make_property!` macro has a case for
properties like that, so in the librsvg source code you will find both of thsese:

```rust
make_property!(
    /// `fill` property.
    ///
    /// https://www.w3.org/TR/SVG/painting.html#FillProperty
    ///
    /// https://www.w3.org/TR/SVG2/painting.html#FillProperty
    Fill,
    default: PaintServer::parse_str("#000").unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
);

make_property!(
    /// `stroke` property.
    ///
    /// https://www.w3.org/TR/SVG2/painting.html#SpecifyingStrokePaint
    Stroke,
    default: PaintServer::None,
    inherits_automatically: true,
    newtype_parse: PaintServer,
);
```

The `newtype_parse:` is what tells the macro that it should generate a newtype like
`struct Stroke(PaintServer)`, and that it should just use the parser that `PaintServer`
already has.

Which parser is that?  Read on.

## Custom parsers

Librsvg has a `Parse` trait for property values which looks rather scary:

```rust
pub trait Parse: Sized {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>>;
}
```

Don't let the lifetimes scare you.  They are required because of `cssparser::Parser`, from
the `cssparser` crate, tries really hard to let you implement zero-copy parsers, which
give you string tokens as slices from the original string being parsed, instead of
allocating lots of little `String` values.  What this `Parse` trait means is, you get
tokens out of the `Parser`, and return what is basically a `Result<Self, Error>`.

In this tutorial we will just show you the parser for simple numeric types, for example,
for properties that can just be represented with an `f64`.  There is the `stroke-miterlimit` property defined like this:

```rust
make_property!(
    /// `stroke-miterlimit` property.
    ///
    /// https://www.w3.org/TR/SVG2/painting.html#StrokeMiterlimitProperty
    StrokeMiterlimit,
    default: 4f64,
    inherits_automatically: true,
    newtype_parse: f64,
);
```

And the `impl Parse for f64` looks like this:

```rust
impl Parse for f64 {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let loc = parser.current_source_location();                                          // (1)
        let n = parser.expect_number()?;                                                     // (2)
        if n.is_finite() {                                                                   // (3)
            Ok(f64::from(n))                                                                 // (4)
        } else {
            Err(loc.new_custom_error(ValueErrorKind::value_error("expected finite number"))) // (5)
        }
    }
}
```

* (1) Store the current location in the parser.

* (2) Ask the parser for a number.  If a non-numeric token comes out (e.g. if the user put `stroke-miterlimit: foo` instead of `stroke-miterlimit: 5`), `expect_number` will return an `Err`, which we propagate upwards with the `?`.

* (3) Check the number for being non-infinite or NaN....

* (4) ... and return the number converted to f64 (`cssparser` returns f32, but we promote them so that subsequent calculations can use the extra precision)...

* (5) ... or return an error based on the location from (1).

My advice: implement new parsers by doing cut&paste from existing ones, and you'll be okay.





* Are you adding support for a CSS property?  Look in the [`property_defs`] and
  [`properties`] modules.  `property_defs` defines most of the CSS properties that librsvg
  supports, and `properties` actually puts all those properties in the `SpecifiedValues`
  and `ComputedValues` structs.

