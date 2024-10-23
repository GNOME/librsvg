CSS Custom Properties — ``var()``
=================================

CSS custom properties, or the ``var()`` feature, let one define named
variables with CSS values that can then be substituted where a property is used.  To quote an example from the spec:

.. code-block:: css

   :root {
     --main-color: #06c;
     --accent-color: #006;
   }
   /* The rest of the CSS file */
   #foo h1 {
     color: var(--main-color);
   }

Additionally, ``var())`` can specify a fallback value, in case the
implementation does not support defining custom properties:

.. code-block:: css

  .foo { color: var(--main-color, #aabbcc); }

In this example, if ``--main-color`` is not defined, it will be
substituted with ``#aabbcc``.


OpenType fonts with SVG data and a color substitution table ("emoji fonts")
---------------------------------------------------------------------------

OpenType allows fonts to define some glyphs in terms of SVG documents.
These glyphs can be recolored: OpenType also allows fonts to have a
color substitution table that will be applied to the SVG.  To do this,
RGB entries in the color table are effectively turned into custom
properties named ``color0``, ``color1``, etc. with RGB values.  Then,
SVG elements specify their colors like ``<path fill="var(--color,
yellow)" d="..." />``, often with a fallback.

OpenType's minimal requirements are that SVG implementations support
``var()`` just in places where a color may be specified (i.e. the
properties that specify SVG paint servers), and that they support the
fallback value.  It does **not** require that implementations actually
support defining custom values for the
``color0``/``color1``/``colorN`` variables, just that fallbacks are used.

This lets librsvg approach supporting CSS custom properties in an
incremental fashion.


Roadmap for incremental support
-------------------------------

* Stage 1 (:issue:`997`): support ``var(--blah, fallback)`` just for
  colors in properties that take paint servers, plus properties like
  ``lightingColor`` (filters) and ``stopColor`` (gradients).  Look in
  ``property_defs.rs`` for places that use the ``Color`` type.  This
  should will make OpenType fonts with color fallbacks work in minimal
  fashion.

* Stage 2 (:issue:`459`): support defining custom properties and
  referencing them.  I wanted to cut&paste Servo's implementation of
  this, but it is a bit involved and may require plenty of refactoring
  to accomodate it from librsvg's code.  If it is too complex, maybe
  we can have a homegrown implementation that just lets one define
  ``--foo: value;`` in a ``:root`` selector, and that just substitutes
  whole values without substitution into other tokens
  (e.g. ``width: var(--some_number)px;`` wouldn't work).

* Stage 3, full support for custom properties with substitution into
  other values.


Letting the caller define values for custom properties
------------------------------------------------------

Adobe's SVG Native Viewer has a `simple API to specify a color map
<https://github.com/adobe/svg-native-viewer/blob/ab9ea1d48b0ff055c2fb063ae4c68edafce5b7c5/svgnative/include/svgnative/SVGDocument.h#L103-L125>`_
that maps string names to RGBA colors.  I think it would be more
future-proof to actually let the caller specify the values in a
``:root`` selector via an external stylesheet; this way we can
accomodate media queries in a clean fashion without growing the public
API.  Media queries are often used to set the custom property values
depending on the media's characteristics (e.g. change colors depending
on dark-mode), and later the properties are used with ``var()``.


Security considerations
-----------------------

If we fully support variable substitution, be careful about the `macro
expansion attack
<https://drafts.csswg.org/css-variables/#long-variables>`_ that can be
done with them.  The spec mentions a mitigation; I think the Servo
code already does this.


References
----------

* Specification: `CSS Custom Properties for Cascading Variables Module Level 1
  <https://drafts.csswg.org/css-variables/#changes>`_

* OpenType specification for `Colors and Color Palettes
  <https://learn.microsoft.com/en-us/typography/opentype/spec/svg#color-and-color-palettes>`_


