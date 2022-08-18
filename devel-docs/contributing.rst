*********************************
How you can contribute to librsvg
*********************************

Here are some projects that you can choose to contribute to librsvg.

Major features
==============

These are big features that may take you a few months to implement.
Fame and glory await! :)

Support CSS var()
-----------------

This project is suitable for a few months of work, like an Outreachy
or Summer of Code internship.  Some familiarity with CSS will be
useful; you don't need to be an expert in it.  You need to be able to
write Rust relatively fluently.

This is https://gitlab.gnome.org/GNOME/librsvg/-/issues/459 - In
summary, support for var() lets CSS styles reference values defined
elsewhere, instead of hard-coding an actual value in every place it is
used.  For example, define a ``button-color`` variable instead of
hard-coding ``#aabbcc`` everywhere.

.. code-block:: css

   /* Define two variables */
   :root {
     --main-color: #06c;
     --accent-color: #006;
   }

   /* The rest of the CSS file */
   #foo h1 {
     fill: var(--main-color);
   }

In the context of librsvg, this would be useful in several situations:

- Allow toolkits to specify a user stylesheet while rendering a
  document, to recolor items in SVGs for icons.  They can already do
  this by using CSS selectors (e.g. change the fill color in elements
  that have the ``button`` class), but CSS ``var()`` would be nice for
  flexibility.

- Part of supporting `SVG Native`_ involves adding support for
  ``var()``.  This is useful for emoji fonts which include glyphs in
  SVG format.

There is a pretty complete in `implementation in Servo`_.  You can use
it for inspiration, or cut&paste it into librsvg; it will need some
changes, and probably also some additions to librsvg's existing
machinery for CSS.

One particularly interesting bit in Servo's implementation is that it guards against exponential expansion of malicious variables.

.. code-block:: css

   :root {
     --prop1: lol;
     --prop2: var(--prop1) var(--prop1);
     --prop3: var(--prop2) var(--prop2);
     --prop4: var(--prop3) var(--prop3);
     /* expand to --prop30 */
   }

A naive implementation would make the consume a few gigabytes of
memory; grep for "exponentially" in Servo's code to see how it guards
against that.

.. _SVG Native: https://gitlab.gnome.org/GNOME/librsvg/-/issues/689

.. _implementation in Servo: https://github.com/servo/servo/blob/master/components/style/custom_properties.rs

Support CSS calc()
------------------

This project is suitable for a few months of work, like an Outreachy
or Summer of Code internship.

Some familiarity with CSS will be useful; you don't need to be an
expert in it.  You need to be able to write Rust relatively fluently.
It will be very useful to have some familiarity with implementing
simple interpreters or evaluators - for example, parsing and computing
an expression like ``5 + 3 * (2 - 4)``; if you have already written a
parser and Abstract Syntax Tree for that kind of thing, this project
will be a lot easier.

This is https://gitlab.gnome.org/GNOME/librsvg/-/issues/843 - support
calc() expressions in CSS.  For example, parameterize an element's
width by using ``width="calc(100% - 40px)"`` (this would use the
viewport's width of 100%, and subtract 40 pixels).

The issue linked in the previous paragraph has plenty of
implementation advice.  It requires some substantial preliminary work,
like completing the separation between specified values and computed
values in librsvg's CSS implementation.  Please ask the maintainer to
prioritize this work if you intend to implement CSS calc().
