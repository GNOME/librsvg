Roadmap
=======

This is an ever-changing list of development priorities for the
maintainers of librsvg.  Check this often!

Short term
----------

- Fix :issue:`778` about incorrect offsetting for layers with opacity.
  Solving this should make it easier to fix the root cause of :issue:`1`, where
  librsvg cannot compute arbitrary regions for filter effects and it only takes the
  user-specified viewport into account.  See :doc:`render_tree` for details on this.

- Continue with the revamp of :doc:`text_layout`.

- Support CSS custom properties ``var()``, at least the minimal
  feature set required for OpenType fonts.  See :doc:`custom_properties`.

- Make fuzzing good and easy - :issue:`1018`.
  See the discussion in that issue for details of the pending work.

Medium term
-----------

- Once we have a :doc:`render_tree` in place (see above), it would be
  convenient if librsvg could generate a tree of paintables for GTK,
  so that GTK could in turn render the SVG with the GPU.  This needs
  detailing in a design document; see :issue:`1140`.

- :issue:`459` - Support CSS ``var()`` for custom colors and other SVG properties.

- :issue:`843` - Support CSS ``calc()``.
