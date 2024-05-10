Roadmap
=======

This is an ever-changing list of development priorities for the
maintainers of librsvg.  Check this often!

Short term
----------

- Fix `issue #778 <https://gitlab.gnome.org/GNOME/librsvg/-/issues/778>`_ about incorrect
  offsetting for layers with opacity.  Solving this should make it easier to fix the root
  cause of `issue #1 <https://gitlab.gnome.org/GNOME/librsvg/-/issues/1>`_, where librsvg
  cannot compute arbitrary regions for filter effects and it only takes the user-specified
  viewport into account.  See :doc:`render_tree` for details on this.

- Continue with the revamp of :doc:`text_layout`.

- Support CSS custom properties ``var()``, at least the minimal
  feature set required for OpenType fonts.  See :doc:`custom_properties`.

- `Make fuzzing good and easy - issue #1018
  <https://gitlab.gnome.org/GNOME/librsvg/-/issues/1018>`_.  See the
  discussion in that issue for details of the pending work.

Medium term
-----------

- `Issue #459 <https://gitlab.gnome.org/GNOME/librsvg/-/issues/459>`_ - Support CSS ``var()`` for custom colors and other SVG properties.

- `Issue #843 <https://gitlab.gnome.org/GNOME/librsvg/-/issues/843>`_ - Support CSS ``calc()``.
