# Text layout in librsvg

This document describes the state of text layout in librsvg as of version 2.52.3,
and how I want to overhaul it completely for SVG2.

## Status as of librsvg 2.52.3

Basic supported features:

* Librsvg supports the elements `text`, `tspan`, `a` inside text, and `tref` (deprecated
  in SVG2, but kept around for SVG1.1 compatibility).  See below for the `x/y/dx/dy`
  attributes; librsvg supports single-number values in these.

* `text-anchor`.

* SVG1.1 values for `direction`, `writing-mode`.  Non-LTR or vertical text layout is very
  much untested.

* SVG1.1 values for `letter-spacing`, `baseline-shift`, `text-decoration`.

* `font` (shorthand), `font-family`, `font-size`, `font-stretch`, `font-style`,
  `font-variant`, `font-weight`.

* `text-rendering`.

Major missing features:

* `text-orientation` and `glyph-orientation-vertical` fallbacks, SVG2 values for `writing-mode`.

* SVG2 `white-space` handling.  This deprecates `xml:space` from SVG1.1.

* Support for multiple values in each of the attributes `x/y/dx/dy` from the `text` and
  `tspan` elements.  Librsvg supports a single value for each attribute, whereas SVG
  allows for multiple values — these then get used to individually position "typographic
  characters" (Pango clusters).  In effect, librsvg's single values for each of those
  attributes mean that each text span can be positioned independently, but not each
  character.

* Relatedly, the `rotate` attribute is not supported.  In SVG it also allows multiple
  values, one for each character.

* `glyph-orientation-vertical` (note that `glyph-orientation-horizontal` is deprecated in SVG2).

* `textPath` is not supported at all.  This will be made much easier by implementing
  `x/y/dx/dy/rotation` first, since each character needs to be positioned and oriented
  individually.

* `@font-face` and WOFF fonts.

Other missing features:

* `display` and `visibility` are not very well tested for the sub-elements of `<text>`.

* SVG2 text with a content area / multi-line / wrapped text: `inline-size`,
  `shape-inside`, `shape-subtract`, `shape-image-threshold`, `shape-margin`,
  `shape-padding`.  This is lower priority than the features above.  Also the related
  properties `text-overflow`,

* `text-align` (shorthand), `text-align-all`, `text-align-last`, `text-indent`, `word-spacing`.

* Baselines: `vertical-align` (shorthand), `dominant-baseline`, `alignment-baseline`,
  `baseline-source`, and SVG2 values for `baseline-shift`.  Note that Pango doesn't
  provide baseline information yet.

* `line-height` (parsed, but not processed).

* SVG2 `text-decoration`, which translates to `text-decoration-line`,
  `text-decoration-style`, `text-decoration-color`.

* `font-feature-settings`, `font-kerning`, `font-size-adjust`.

* CSS Text 3/4 features not mentioned here.

Features that will not be implemented:

* SVG1.1 features like `<font>` and the `glyph-orientation-horizontal` property, that were
  deprecated for SVG2.

## Roadmap summary

Since librsvg 2.52.1 I've started to systematically improve text support.  Many thanks to
Behdad Esfahbod, Khaled Ghetas, Matthias Clasen for their advice and inspiration.

First, I want to get **bidi** to a state where it is reliable, at least as much as LTR
languages with Latin text are reliable right now:

* Implement `unicode-bidi`.  See the detailed roadmap below.

* Add tests for the different combinations of `text-anchor` and `direction`; right now
  there are only a few tested combinations.

* Test and implement multiply-nested changes of direction.  I think
  only a single level works right now.

* Even if white-space handling remains semi-broken, I think it's more important to have
  "mostly working" bidi than completely accurate white-space handling and layout.

Second, actually overhaul librsvg's text engine by implementing the SVG2 text layout algorithm:

* Implement the `text-orientation` property, and implement fallbacks from the deprecated
  `glyph-orientation-vertical` to it.  If this turns out to be hard with the current state
  of the code, I will defer it until the SVG2 text layout algorithm below.

* Implement the SVG2 text layout algorithm and `white-space` handling at the same time.
  See the detailed roadmap below.

Third, implement all the properties that are not critical for the text layout algorithm,
and things like `@font-face`.  Those can be done gradually, but I feel the text layout
algorithm has to be done all in a single step.

## Detailed roadmap

### Implement `unicode-bidi`

The property is parsed only with SVG1.1 values.  Parsing SVG2 values is a trivial change.
Supporting this property involves looking at both `direction` and `unicode-bidi` and
inserting Unicode control characters at the start and end of each text span, so that the
bidi and shaping engines know what to do.

### Add tests for combinations of `text-anchor` and `direction`

These are easy to add now that librsvg's tests make use of the Ahem font, in which each
glyph is a 1x1 em square.

### Implement the `text-orientation` property

This may just be the property parser and hooking it up to the machinery for properties.
Actual processing may be easier to do in the SVG2 text layout algorithm, detailed below.

### Implement the SVG2 text layout algorithm and `white-space` handling.

**Shaping:** One thing librsvg does wrong is that for each `<tspan>`, or for each
synthesized text span from a `<text>` element, it creates a separate `pango::Layout`.
This means that text shaping is not done across element boundaries (SVG2 requirement).
Implementing this can be done by creating a string by recursively concatenating the
character content of each `<text>` element and its children, and adding
`pango::Attribute`s with the proper indexes based on each child's character length.  This
creates an un-shaped string in logical order with all the characters inside the `<text>`,
to be used in the next steps.

Pango details: create a single `pango::Layout`, per `<text>` element, with
`pango::Attribute` for each text span.  Set the layout to `set_single_paragraph_mode()` so
it does not break newlines.  Pango will then translate them to   characters in the
`Layout`, and the white-space handling and SVG2 text layout algorithm below can detect
them.

**Bidi control:** The `unicode-bidi` property requires adding control characters at the
start and end of each span's text contents.  For example, `<tspan direction="rtl"
unicode-bidi="bidi-override">foo</tspan>` should get rendered as `oof`.  The CSS Writing
Modes 3 spec has a [table of control
codes](https://www.w3.org/TR/css-writing-modes-3/#unicode-bidi) for each combination of
`direction` and `unicode-bidi`.  Implementing this involves adding the control characters
while recursively building the string from each child of `<text>` as in the "Shaping"
point above.

**White-space handling:** SVG2 has a new `white-space` property that obsoletes `xml:space`
from SVG1.1.  Implementing this depends on the concatenated string from the steps above,
so that white-space can be collapsed on the result.  Maybe this needs to be done before
inserting bidi control characters, or maybe not, if the state machine is adjusted to
ignore the control characters.

**SVG2 text layout algorithm:** This is the big one.  The spec has pseudocode.  It depends
on the shaping results from Pango, and involves correlating "typographic characters"
(Pango clusters) with the un-shaped string in logical order from the "Shaping", and the
information about discarded white-space characters.  The complete text layout algorithm
would take care of supporting multi-valued `x/y/dx/dy/rotate`, `textPath` (see below),
plus bidi and vertical text.

### Text rendering

Librsvg is moving towards a "render tree" or "display list" model, instead of just
rendering everything directly while traversing the DOM tree.

Currently, the text layout process generates a `layout::Text` object, which is basically
an array of `pango::Layout` with extra information.

It should be possible to explode these into `pango::GlyphItem` or `pango::GlyphString` and
annotate these with `x/y/rotate` information, which will be the actual results of the SVG2
text layout algorithm.

Although currently Pango deals with underlining, it may be necessary to do that in librsvg
instead - I am not sure yet how `textPath` or individually-positioned `x/y/dx/dy/rotate`
interact with underlining.

### Pango internals

```
/**
 * pango_renderer_draw_glyph_item:
 * @renderer: a `PangoRenderer`
 * @text: (nullable): the UTF-8 text that @glyph_item refers to
 * @glyph_item: a `PangoGlyphItem`
 * @x: X position of left edge of baseline, in user space coordinates
 *   in Pango units
 * @y: Y position of left edge of baseline, in user space coordinates
 *   in Pango units
 *
 * Draws the glyphs in @glyph_item with the specified `PangoRenderer`,
 * embedding the text associated with the glyphs in the output if the
 * output format supports it.
 *
 * This is useful for rendering text in PDF.
 * ...
 */
```

Note that embedding text in PDF to make it selectable involves passing
a non-null `text` to pango_renderer_draw_glyph_item().  We'll have to
implement this by hand, probably.

### Wrapped text in a content area

This roadmap does not consider the implementation fo wrapped text yet.

### User-provided fonts, `@font-face` and WOFF

This involves changes to the CSS machinery, to parse the `@font-face` at-rule.  Librsvg
would also have to obtain the font and feed it to FontConfig.  I am not sure if FontConfig
can deal with WOFF just like with normal `.ttf` files.

## Issues

https://gitlab.gnome.org/GNOME/librsvg/-/issues/795 - Implement the unicode-bidi property.

https://gitlab.gnome.org/GNOME/librsvg/-/issues/795 - Implement SVG2 white-space behavior.

https://gitlab.gnome.org/GNOME/librsvg/-/issues/599 - Something is wrong with text scaled
with a transformation; this is not critical but it bothers me a lot.

### Issues that have not been filed yet

From the spec: "It is possible to apply a gradient, pattern, clipping path, mask or filter
to text."  We need better tests for the objectBoundingBox of the whole `<text>`; I think
they are wrong for vertical text, and this shows up when filling its spans with gradients
or patterns.  Clip/mask/filter do not work on individual spans yet.

Multiply-nested changes of text direction / bidi overrides.

## Glossary so I don't have to check the Pango docs every time

PangoItem - A range within the user's string that has the same
language/script/direction/level/etc. (Logical order).

PangoLayoutRun - same as PangoGlyphItem - a pair of PangoItem and the PangoGlyphString it
generated during shaping. (Visual order).

PangoGlyphString - The glyphs generated for a single PangoItem.

PangoGravityHint - Defines how horizontal scripts should behave in a vertical context.
