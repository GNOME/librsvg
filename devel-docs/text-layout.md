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

## Issues

https://gitlab.gnome.org/GNOME/librsvg/-/issues/795 - Implement the unicode-bidi property.

https://gitlab.gnome.org/GNOME/librsvg/-/issues/795 - Implement SVG2 white-space behavior.
