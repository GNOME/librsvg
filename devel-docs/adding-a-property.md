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




* Are you adding support for a CSS property?  Look in the [`property_defs`] and
  [`properties`] modules.  `property_defs` defines most of the CSS properties that librsvg
  supports, and `properties` actually puts all those properties in the `SpecifiedValues`
  and `ComputedValues` structs.

