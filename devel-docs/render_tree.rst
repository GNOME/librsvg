Render tree
===========

As of 2022/Oct/06, librsvg does not compute a render tree data
structure prior to rendering.  Instead, in a very 2000s fashion, it
walks the tree of elements and calls a ``.draw()`` method for each
one.  Each element then calls whatever methods it needs from
``DrawingCtx`` to draw itself.  Elements which don't produce graphical
output (e.g. ``<defs>`` or ``<marker>``) simply have an empty
``draw()`` method.

Over time we have been refactoring that in the direction of actually
being able to produce a render tree.  What would that look like?
Consider an SVG document like this:

.. code-block:: xml
   
   <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
     <defs>
       <rect id="TheRect" x="10" y="10" width="20" height="20" fill="blue"/>
     </defs>
   
     <g>
       <use href="#TheRect" stroke="red" stroke-width="2"/>
   
       <circle cx="50" cy="50" r="20" fill="yellow"/>
     </g>
   </svg>

A render tree would be a list of nested instructions like this:

::

   group {                            # refers to the toplevel SVG
     width: 100
     height: 100
     establishes_viewport: true       # because it is an <svg> element

     children {
       group {                        # refers to the <g>
         establishes_viewport: false  # because it is a simple <g>

         children {
           shape {
             path="the <rect> above but resolved to path commands"
    
             # note how the following is the cascaded style and the <use> semantics
             fill: blue
             stroke: red
             stroke-width: 2
           }
    
           shape {
             path="the <circle> above but resolved to path commands"
    
             fill: yellow
           }
         }
       }
     }
   }

That is, we take the high-level SVG instructions and "lower" them to a
few possible drawing primitives like path-based shapes that can be
grouped.  All the primitives have everything that is needed to draw
them, like their set of computed values for styles, and their
coordinates resolved to their user-space coordinate system.

Browser engines produce render trees more or less similar to the above
(they don't always call them that), and get various benefits:

- The various recursively-nested subtrees can be rendered concurrently.

- Having low-level primitives makes it easier to switch to another
  rendering engine in the future.

- The tree can be re-rendered without recomputation, or subtrees can
  be recomputed efficiently if e.g. an animated element changes a few
  of its properties.

Why did librsvg not do that since the beginning?
------------------------------------------------

Librsvg was originally written in the early 2000s, when several things
were happening at the same time:

- libxml2 (one of the early widely-available parsers for XML) had
  recently gotten a SAX API for parsing XML.  This lets an application
  stream in the parsed XML elements and process them one by one,
  without having to build a tree of elements+attributes first.  In
  those days, memory was at a premium and "not producing a tree" was
  seen as beneficial.

- The SVG spec itself was being written, and it did not have all of
  the features we know now.  In particular, maybe at some point it
  didn't have elements that worked by referencing others, like
  ``<use>`` or ``<filter>``.  The CSS cascade could be done on the fly
  for the XML elements being streamed in, and one could emit rendering
  commands for each element to produce the final result.

That is, at that time, it was indeed feasible to do this: stream in
parsed XML elements one by one as produced by libxml2, and for each
element, compute its CSS cascade and render it.

This scheme probably stopped working at some point when SVG got
features that allowed referencing elements that have not been declared
yet (think of ``<use href="#foo"/>`` but with the ``<defs> <path
id="foo" .../> </defs>`` declared until later in the document).  Or
elements that referenced others, like ``<rect filter="url(#blah)">``.
In both cases, one needs to actually build an in-memory tree of parsed
elements, and *then* resolve the references between them.

That is where much of the complexity of librsvg's code flow comes from:

- ``AcquiredNodes`` is the thing that resolves references when needed.
  It also detects reference cycles, which are an error.

- ``ComputedValues`` often get resolved until pretty late, by passing
  the ``CascadedValues`` state down to children as they are drawn.

- ``DrawingCtx`` was originally a giant ball of mutable state, but we
  have been whittling it down and moving part of that state elsewhere.


Summary of the SVG rendering model
----------------------------------

FIXME:

paint
clip
mask
filter
composite


Current state
-------------

``layout.rs`` has the beginnings of the render tree.  It's probably mis-named?  It contains this:

- A primitive for path-based shapes.

- A primitive for text.

- A stacking context, which indicates each layer's opacity/clip/mask/filters.

- Various ancillary structures that try to have only user-space
  coordinates (e.g. a number of CSS pixels instead of ``5cm``) and no
  references to other things.

The last point is not yet fully realized.  For example,
``StackingContext.clip_in_user_space`` has a reference to an element,
which will be used as the clip path — that one needs to be normalized
to user-space coordinates in the end.  Also,
``StackingContext.filter`` is a filter list as parsed from the SVG,
not a ``FilterSpec`` that has been resolved to user space.

It would be good to resolve everything as early as possible to allow
lowering concepts to their final renderable form.  Whenever we have
done this via refactoring, it has simplified the code closer to the
actual rendering via Cairo.

Major subprojects
-----------------

Path based shapes (``layout::Shape``) and text primitives
(``layout::Text``) are almost done.  The only missing thing for shapes
would be to "explode" their markers into the actual primitives that
would be rendered for them.  However...

There is no primitive for groups yet.  Every SVG element that allows
renderable children must produce a group primitive of some sort:
``svg``, ``g``, ``use``, ``marker``, etc.  Among those, ``use` and
``marker`` are especially interesting since they must explode their
referenced subtree into a shadow DOM, which librsvg doesn't support
yet for CSS cascading purposes (the reference subtree gets rendered
properly, but the full semantics of shadow DOM are not implemented
yet).

Elements that establish a viewport (``svg``, ``symbol``, ``image``,
``marker``, ``pattern``) need to carry information about this
viewport, which is a ``viewBox`` plus ``preserveAspectRatio``.  See #298.

The ``layout::StackingContext`` struct should contain another field,
probably called ``layer``, with something like this:

.. code-block:: rust

   struct StackingContext {
       // ... all its current fields

       layer: Layer
   }
                
   enum Layer {
       Shape(Box<Shape>),
       Text(Box<Text>),
       StackingContext(Box<StackingContext>)
   }

That is, every stacking context should contain the thing that it will
draw, and that thing may be a shape/text or another stacking context!

Bounding boxes
--------------

SVG depends on the ``objectBoundingBox`` of an element in many places:
to resolve a gradient's or pattern's units, to determine the size of
masks and clips, to determine the size of the filter region.

The current big bug to solve is #778, which requires knowing the
``objectBoundingBox`` of an element **before** rendering it, so that a
temporary surface of the appropriate size can be created for rendering
the element if it has isolated opacity or masks/filters.  Currently
librsvg creates a temporary surface with the size and position of the
toplevel viewport, and this is wrong for shapes that fall outside the
viewport.

The problem is that librsvg computes bounding boxes at the time of
rendering, not before that.  However, now ``layout::Shape`` and
``layout::Text`` already know their bounding box beforehand.  Work
needs to be done to do the same for a ``layout::Group`` or whatever
that primitive ends up being called (by taking the union of its
children's bounding boxes, so e.g. that a group with a filter can
create a temporary surface to be able to render all of its children
and then filter the surface).
