Viewport abstraction
====================

**Status:** implemented as of 2025/Apr/09.

:issue:`298` is about unifying several concepts into a Viewport abstraction.
This chapter attempts to explain that.

SVG has the concept of `establishing a new viewport
<https://svgwg.org/svg2-draft/coords.html#EstablishingANewSVGViewport>`_,
in particular for the elements ``<svg>`` and ``<symbol>``.  The
viewport is set up like this:

- Apply the element's ``transform``.

- Compute a transform / coordinate system from the element's
  size+position (``x``, ``y``, ``width``, ``height``), the
  ``preserveAspectRatio`` and ``viewBox`` attributes.

- Set up a clipping rectangle if the element's ``overflow`` property
  says so.

However, that mechanism is general enough that it can also be made to
work when rendering the elements ``<image>``, ``<marker>``, and
``<pattern>``.  They have their own way of specifying a size (e.g. the
marker-specific ``markerWidth`` and ``markerHeight`` attributes), but
they also need to compute a new transform, set up clipping, etc.

The original code for librsvg reimplemented the mechanism above
independently for each of ``<marker>``, ``<symbol>``, etc., by doing
direct calls to Cairo to set up a transform and a clipping rectangle.
Unfortunately, during the initial port to Rust we did not identify
this pattern to gather the various implementations and abstract them
in a single place.  Gradual refactoring led to all calls to Cairo
happening in ``drawing_ctx.rs``, instead of all over the code.  Still,
the various versions still exist, with slightly different mechanisms
for each.

What I'd like to do
-------------------

The idea in #298 is to consolidate all the parameters needed for a
viewport, as mentioned above, into a single place.  Now that the
structs for a :doc:`render_tree` are starting to take hold, I think we
can do these:

- Move the ``Viewport`` struct from ``drawing_ctx.rs`` into ``layout.rs``.

- Add the necessary fields from the previous section (element's
  transform, perhaps moved from the ``StackingCtx``), the viewport
  size, ``preserveAspectRatio``, and ``overflow``.

- (Look at the Firefox source code a bit before doing that; they have
  nice code for it.)

- One by one, migrate each part of librsvg that requires it to using
  the new ``Viewport`` abstraction with everything in it.  We can
  probably start with ``<svg>`` and ``<use> / <symbol>``; markers and
  patterns may need a little extra untangling.

