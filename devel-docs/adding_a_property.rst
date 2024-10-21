How to add a new CSS property
=============================

This document is a little tour on how to add support for a CSS property
to librsvg. We will implement the |mask-type|_ property from the
**CSS Masking Module Level 1** specification.

What is ``mask-type``?
----------------------

The spec says about |mask-type|_:

   The mask-type property defines whether the content of the mask
   element is treated as as luminance mask or alpha mask, as described
   in Calculating mask values.

A **luminance mask** takes the RGB values of each pixel, converts them
to a single luminance value, and uses that as a mask.

An **alpha mask** just takes the alpha value of each pixel and uses it
as a mask.

The only mask type that SVG1.1 supported was luminance masks; there
wasn’t even a ``mask-type`` property back then. The SVG2 spec removed
descriptions of masking, and offloaded them to the `CSS Masking Module
Level 1 <https://www.w3.org/TR/css-masking-1/>`__ specification, which
it adds the ``mask-type`` property and others as well.

Let’s start by figuring out how to read the spec.

What the specification says
---------------------------

The specification for ``mask-type`` is in
https://www.w3.org/TR/css-masking-1/#the-mask-type

In the specs, most of the descriptions for properties start with a table
that summarizes the property. For example, if you visit that link, you
will find a table that starts with these items:

-  **Name:** ``mask-type``
-  **Value:** ``luminance | alpha``
-  **Initial:** ``luminance``
-  **Applies to:** mask elements
-  **Inherited:** no
-  **Computed value:** as specified

Let’s go through each of these:

**Name:** We have the name of the property (``mask-type``). Properties
are case-insensitive, and librsvg already has machinery to handle that.

**Value:** The possible values for the property can be ``luminance`` or
``alpha``. In the spec’s web page, even the little ``|`` between those
two values is a hyperlink; clicking it will take you to the
specification for CSS Values and Units, where it describes the grammar
that the CSS specs use to describe their values. Here you just need to
know that ``|`` means that exactly one of the two alternatives must
occur.

As you may imagine, librsvg already parses a lot of similar properties
that are just symbolic values. For example, the ``stroke-linecap``
property can have values ``butt | round | square``. We’ll see how to
write a parser for this kind of property with a minimal amount of code.

**Initial:** Then there is the initial or default value, which is
``luminance``. This means that if the ``mask-type`` property is not
specified on an element, it takes ``luminance`` as its default. This is
a sensible choice, since an SVG1.1 file that is processed by SVG2
software should retain the same semantics. It also means that if there
is a parse error, for example if you typed ``ahlpha``, the property will
silently revert back to the default ``luminance`` value.

**Applies to:** Librsvg doesn’t pay much attention to “applies to” — it
just carries property values for all elements, and the elements that
don’t handle a property just ignore it.

**Inherited:** This property is not inherited, which means that by
default, its value does not cascade. So if you have this:

.. code:: xml

   <mask style="mask-type: alpha;">
     <other>
       <elements>
         <here/>
       </elements>
     </other>
   </mask>

Then the ``other``, ``elements``, ``here`` will not inherit the
``mask-type`` value from their ancestor.

**Computed value:** Finally, the computed value is “as specified”, which
means that librsvg does not need to modify it in any way when resolving
the CSS cascade. Other properties, like ``width: 1em;`` may need to be
resolved against the ``font-size`` to obtain the computed value.

The W3C specifications can get pretty verbose and it takes some practice
to read them, but fortunately this property is short and sweet.

Let’s go on.

How librsvg represents properties
---------------------------------

Each property has a Rust type that can hold its values. Remember the
part of the masking spec from above, that says the ``mask-type``
property can have values ``luminance`` or ``alpha``, and the
initial/default is ``luminance``? This translates easily to Rust types:

.. code:: rust

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

Additionally, we need to be able to say that the property does not
inherit by default, and that its computed value is the same as the
specified value (e.g. we can just copy the original value without
changing it). Librsvg defines a ``Property`` trait for those actions:

.. code:: rust

   pub trait Property {
       fn inherits_automatically() -> bool;

       fn compute(&self, _: &ComputedValues) -> Self;
   }

For the ``mask-type`` property, we want ``inherits_automatically`` to
return ``false``, and ``compute`` to return the value unchanged. So,
like this:

.. code:: rust

   impl Property for MaskType {
       fn inherits_automatically() -> bool {
           false
       }

       fn compute(&self, _: &ComputedValues) -> Self {
           self.clone()
       }
   }

Ignore the ``ComputedValues`` argument for now — it is how librsvg
represents an element’s complete set of property values.

As you can imagine, there are a lot of properties like ``mask-type``,
whose values are just symbolic names that map well to a data-less enum.
For all of them, it would be a lot of repetitive code to define their
default value, return whether they inherit or not, and clone them for
the computed value. Additionally, we have not even written the parser
for this property’s values yet.

Fortunately, librsvg has a ``make_property!`` macro that lets you do
this instead:

.. code:: rust

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

-  

   (1) is a documentation comment for the ``MaskType`` enum being
       defined.

-  

   (2) is ``MaskType``, the name we will use for the ``mask-type``
       property.

-  

   (3) indicates the “initial value”, or default, for the property.

-  

   (4) … whether the spec says the property should inherit or not.

-  

   (5) Finally, ``identifiers:`` is what makes the ``make_property!``
       macro know that it should generate a parser for the symbolic
       names ``luminance`` and ``alpha``, and that they should
       correspond to the values ``MaskType::Luminance`` and
       ``MaskType::Alpha``, respectively.

This saves a lot of typing! Also, it makes it easier to gradually change
the way properties are represented, as librsvg evolves.

Properties that use the same data type
--------------------------------------

Consider the ``stroke`` and ``fill`` properties; both store a |<paint>|_
value, which librsvg represents with a type called ``PaintServer``. The
``make_property!`` macro has a case for properties like that, so in the
librsvg source code you will find both of thsese:

.. code:: rust

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

The ``newtype_parse:`` is what tells the macro that it should generate a
newtype like ``struct Stroke(PaintServer)``, and that it should just use
the parser that ``PaintServer`` already has.

Which parser is that? Read on.

Custom parsers
--------------

Librsvg has a ``Parse`` trait for property values which looks rather
scary:

.. code:: rust

   pub trait Parse: Sized {
       fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>>;
   }

Don’t let the lifetimes scare you. They are required because of
``cssparser::Parser``, from the ``cssparser`` crate, tries really hard
to let you implement zero-copy parsers, which give you string tokens as
slices from the original string being parsed, instead of allocating lots
of little ``String`` values. What this ``Parse`` trait means is, you get
tokens out of the ``Parser``, and return what is basically a
``Result<Self, Error>``.

In this tutorial we will just show you the parser for simple numeric
types, for example, for properties that can just be represented with an
``f64``. There is the ``stroke-miterlimit`` property defined like this:

.. code:: rust

   make_property!(
       /// `stroke-miterlimit` property.
       ///
       /// https://www.w3.org/TR/SVG2/painting.html#StrokeMiterlimitProperty
       StrokeMiterlimit,
       default: 4f64,
       inherits_automatically: true,
       newtype_parse: f64,
   );

And the ``impl Parse for f64`` looks like this:

.. code:: rust

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

-  

   (1) Store the current location in the parser.

-  

   (2) Ask the parser for a number. If a non-numeric token comes out
       (e.g. if the user put ``stroke-miterlimit: foo`` instead of
       ``stroke-miterlimit: 5``), ``expect_number`` will return an
       ``Err``, which we propagate upwards with the ``?``.

-  

   (3) Check the number for being non-infinite or NaN….

-  

   (4) … and return the number converted to f64 (``cssparser`` returns
       f32, but we promote them so that subsequent calculations can use
       the extra precision)…

-  

   (5) … or return an error based on the location from (1).

My advice: implement new parsers by doing cut&paste from existing ones,
and you’ll be okay.

Registering the property
------------------------

Okay! We defined ``MaskType`` and its symbolic identifiers with the
``make_property!`` macro, and the macro took care of writing a parser
for it and implementing the traits that the property needs.

Now we need to modify the code in a few places to process the property.

Register the property
---------------------

-  First, look for ``longhands:`` in ``properties.rs``. You will find
   that it is part of a long macro invocation:

.. code:: rust

   make_properties! {
       // ... stuff omitted here

       longhands: {
          // ... stuff omitted here

           "marker-end"                  => (PresentationAttr::Yes, marker_end                  : MarkerEnd),
           "marker-mid"                  => (PresentationAttr::Yes, marker_mid                  : MarkerMid),
           "marker-start"                => (PresentationAttr::Yes, marker_start                : MarkerStart),
           "mask"                        => (PresentationAttr::Yes, mask                        : Mask),
           // "mask-type"                => (PresentationAttr::Yes, unimplemented),
           "opacity"                     => (PresentationAttr::Yes, opacity                     : Opacity),
           "overflow"                    => (PresentationAttr::Yes, overflow                    : Overflow),

           // ... stuff omitted here
       }
   }

In there, there is an entry for ``mask-type`` commented out. Let’s
uncomment it and turn it into this:

.. code:: rust

           "mask-type"                   => (PresentationAttr::Yes, mask_type                   : MaskType),

``PresentationAttr::Yes`` indicates whether the property has a
corresponding presentation attribute. This means that you can do
``<mask style="mask-type: alpha;">`` which is property, as well as
``<mask mask-type="alpha">``, which is a presentation attribute.

How did we find out that ``mask-type`` also exists as a presentation
attribute? Well, `the spec
<https://www.w3.org/TR/css-masking-1/#the-mask-type>`__ says:

   The mask-type property is a presentation attribute for SVG elements.

But wait! If we compile, we get this:

::

   error: no rules expected the token `"mask-type"`
      --> src/properties.rs:450:9
       |
   450 |         "mask-type"                   => (PresentationAttr::Yes, mask_type                   : MaskType),
       |         ^^^^^^^^^^^ no rules expected this token in macro call

When you see that error in exactly that macro invocation, it means this:
librsvg uses a crate called ``markup5ever`` to have a compact
representation of the names of properties/attributes/elements. It uses
string interning so that, for example, there is a single definition of
``rect`` in the program’s heap instead of there being a thousands of
duplicated ``rect`` strings when you load a big document. The thing is,
``markup5ever`` only has ready-made definitions of the most common
HTML/SVG/CSS names, but unfortunately ``mask-type`` is not one of them.

So, we scroll down in ``properties.rs`` and move the ``mask-type``
registration there:

.. code:: rust

       longhands_not_supported_by_markup5ever: {
           "line-height"                 => (PresentationAttr::No,  line_height                 : LineHeight),
           "mask-type"                   => (PresentationAttr::Yes, mask_type                   : MaskType),     // <- right here
           "mix-blend-mode"              => (PresentationAttr::No,  mix_blend_mode              : MixBlendMode),
           "paint-order"                 => (PresentationAttr::Yes, paint_order                 : PaintOrder),
       }

That block named ``longhands_not_supported_by_markup5ever`` is, well,
exactly what it says — a separate section with property names that are
not built into ``markup5ever``, so they must be dealt with specially.
Just put the property there and that’s it.

Next, we have to calculate the computed value for the property.

Calculate the computed value
----------------------------

In ``properties.rs``, look for ``compute!``. You will find many
invocations of this macro:

.. code:: rust

           compute!(MarkerEnd, marker_end);
           compute!(MarkerMid, marker_mid);
           compute!(MarkerStart, marker_start);
           compute!(Mask, mask);
           compute!(MixBlendMode, mix_blend_mode);
           compute!(Opacity, opacity);
           compute!(Overflow, overflow);

Add a call for ``MaskType``:

.. code:: rust

           compute!(MarkerEnd, marker_end);
           compute!(MarkerMid, marker_mid);
           compute!(MarkerStart, marker_start);
           compute!(Mask, mask);
           compute!(MaskType, mask_type);          // this is new
           compute!(MixBlendMode, mix_blend_mode);
           compute!(Opacity, opacity);
           compute!(Overflow, overflow);

You will see that all those calls to ``compute!`` are inside a method
called ``SpecifiedValues::to_computed_values()``. This method is run as
part of the CSS cascade: it takes the ``SpecifiedValues`` from an
element and composes them onto the ``ComputedValues`` from its parent
element. For example, if you have a document with this bit:

.. code:: xml

   <g stroke="red" fill="blue">     // ComputedValues with stroke:red, fill:blue
     <rect fill="green"/>           // SpecifiedValues with fill:green
   </g>

The ``ComputedValues`` that results from the ``<g>`` will have
properties ``stroke:red`` and ``fill:blue`` in it. The
``SpecifiedValues`` from the ``<rect>`` just has ``fill:green``.
Composing them together for the ``<rect>`` gives us ``ComputedValues``
with ``stroke:red`` and ``fill:green``.

Now that the property is registered, we can actually handle it in the
drawing code!

Handling the property
---------------------

First, a digression: let’s change the name of a few methods to better
reflect what the new structure of the code will be like.

There are a few methods called ``to_mask`` in the code, that take an
RGBA surface and turn it into an Alpha-only surface with the luminance
of the original surface; and also the corresponding method to do this
for a single pixel. Let’s do this kind of renaming:

::

   -    pub fn to_mask(&self, opacity: UnitInterval) -> Result<SharedImageSurface, cairo::Error> {
   +    pub fn to_luminance_mask(&self, opacity: UnitInterval) -> Result<SharedImageSurface, cairo::Error> {

Librsvg only effectively supported ``mask-type: luminance`` since that
is what was in SVG1.1, but now for SVG2 we want to add behavior for
``mask-type: alpha`` as well. So, it makes sense to rename ``to_mask``
as ``to_luminance_mask``.

``SharedImageSurface`` is the type that librsvg uses to represent images
in memory. They can be RGBA or Alpha-only. There is already a method
called ``extract_alpha`` that we can use to create an Alpha-only mask:

.. code:: rust

   // there's a type alias SharedImageSurface for this
   impl ImageSurface<Shared> {
       pub fn extract_alpha(&self, bounds: IRect) -> Result<SharedImageSurface, cairo::Error> { ... }
   }

Now let’s look at where ``drawing_ctx.rs`` has this:

.. code:: rust

           let mask = SharedImageSurface::wrap(mask_content_surface, SurfaceType::SRgb)?    // (1)
               .to_luminance_mask()?                                                        // (2)
               .into_image_surface()?;                                                      // (3)

-  

   (1) Wraps a ``SharedImageSurface`` around the Cairo surface that was
       just rendered with the mask contents.

-  

   (2) Converts it to a luminance mask. We will need to change this!

-  

   (3) Extracts the Cairo image surface from the ``SharedImageSurface``,
       for further processing.

Remember the ``ComputedValues`` where we had the ``mask_type``? We can
extract it with ``values.mask_type()``. Now let’s change the lines above
to this:

.. code:: rust

           let tmp = SharedImageSurface::wrap(mask_content_surface, SurfaceType::SRgb)?;

           let mask_result = match values.mask_type() {
               MaskType::Luminance => tmp.to_luminance_mask()?,
               MaskType::Alpha => tmp.extract_alpha(IRect::from_size(tmp.width(), tmp.height()))?,
           };

           let mask = mask_result.into_image_surface()?;

But wait! We don’t have a test for this yet! Aaaaaargh, we are doing
test-driven development backwards!

No biggie. Let’s write the tests.

Adding tests
------------

Testing graphical output is really annoying if you compare PNG files,
because any time Cairo changes something and antialiasing changes
juuuuuust a bit, the tests break. So, librsvg tries to do “reftests”, or
reference tests, by comparing the rendered results of two things:

-  The SVG you actually want to test.
-  An equivalent SVG that works only with known-good features.

For ``mask-type``, we need an SVG document that actually uses that
property with both of its values, and another document that produces the
same results but with simpler primitives.

Librsvg already has tests for luminance masks, as they were the only
available kind in SVG1.1. So we can be confident that they already work
- we just need to test that the presence of ``mask-type="luminance"``
actually does the same thing.

First, let’s dissect the SVG that we want to test:

.. code:: xml

   <?xml version="1.0" encoding="UTF-8"?>
   <svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
     <mask id="luminance" mask-type="luminance" maskContentUnits="objectBoundingBox">
       <rect x="0.1" y="0.1" width="0.8" height="0.8" fill="white"/>
     </mask>
     <mask id="alpha" mask-type="alpha" maskContentUnits="objectBoundingBox">
       <rect x="0.1" y="0.1" width="0.8" height="0.8" fill="black"/>
     </mask>

     <rect x="0" y="0" width="100" height="100" fill="green" mask="url(#luminance)"/>

     <rect x="100" y="0" width="100" height="100" fill="green" mask="url(#alpha)"/>
   </svg>

The image has two 100x100 ``green`` squares side by side. The one on the
left gets masked with the ``luminance`` mask, which reduces it to an
80x80 rectangle. That mask is a **white** square, so its has full
luminance at every pixel.

The square on the right gets masked with the ``alpha`` mask. That mask
is a **black** square, but with alpha=1.0, so it should produce the same
result as the first one.

Note that to make things easy, we use **white** for the luminance mask.
White pixels have full luminance (1.0), which gets used as the mask.
Conversely, we use **black** for the alpha mask. Those black pixels are
fully opaque, and since ``mask-type="alpha"`` only considers the alpha
channel, it will be using the full opacity of each pixel (1.0), which
also gets used as the mask. So, the masks should be equivalent.

Okay! Now let’s write the reference SVG, the one built out of simpler
elements but that should produce the same rendering:

.. code:: xml

   <?xml version="1.0" encoding="UTF-8"?>
   <svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
     <rect x="10" y="10" width="80" height="80" fill="green"/>

     <rect x="110" y="10" width="80" height="80" fill="green"/>
   </svg>

This is just the two original squares, but already clipped or masked to
the final result.

Now, where do we put those SVG documents for the tests?

Near the end of ``tests/src/filters.rs`` we can include this:

.. code:: rust

   test_compare_render_output!(
       mask_type,
       200,
       100,
       br##"<?xml version="1.0" encoding="UTF-8"?>
   <svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
     <mask id="luminance" mask-type="luminance" maskContentUnits="objectBoundingBox">
       <rect x="0.1" y="0.1" width="0.8" height="0.8" fill="white"/>
     </mask>
     <mask id="alpha" mask-type="alpha" maskContentUnits="objectBoundingBox">
       <rect x="0.1" y="0.1" width="0.8" height="0.8" fill="black"/>
     </mask>

     <rect x="0" y="0" width="100" height="100" fill="green" mask="url(#luminance)"/>

     <rect x="100" y="0" width="100" height="100" fill="green" mask="url(#alpha)"/>
   </svg>
   "##,
       br##"<?xml version="1.0" encoding="UTF-8"?>
   <svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
     <rect x="10" y="10" width="80" height="80" fill="green"/>

     <rect x="110" y="10" width="80" height="80" fill="green"/>
   </svg>
   "##,
   );

Here, ``test_compare_render_output!`` is a macro that takes two SVG
documents, the test and the reference, and compares their rendered
results. It also takes a test name (``mask_type`` in this case), and the
pixel size of the image to generate for testing (200x100).

Final steps: documentation
--------------------------

To help people who are wondering what SVG features are supported in
librsvg, there is a ``FEATURES.md`` file. It has a section called “CSS
properties” with a big list of property names and notes about them.

We’ll patch it like this:

::

    | marker-mid                  |                                                        |
    | marker-start                |                                                        |
    | mask                        |                                                        |
   +| mask-type                   |                                                        |
    | mix-blend-mode              | Not available as a presentation attribute.             |
    | opacity                     |                                                        |
    | overflow                    |                                                        |

There is nothing remarkable about ``mask-type``, it is a plain old
property that also has a presentation attribute (remember the
``PresentationAttr::Yes`` from above?), so we don’t need to list any
extra information.

And with that, we are done implementing ``mask-type``. Have fun!



.. See https://docutils.sourceforge.net/FAQ.html#is-nested-inline-markup-possible

.. |mask-type| replace:: ``mask-type``
.. _mask-type: https://www.w3.org/TR/css-masking-1/#the-mask-type

.. |<paint>| replace:: ``<paint>``
.. _<paint>: https://www.w3.org/TR/SVG2/painting.html#SpecifyingPaint
