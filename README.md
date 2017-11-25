Librsvg
=======

This is librsvg - A small SVG rendering library associated with the
[GNOME Project][gnome].  It renders SVG files to [Cairo][cairo]
surfaces.  Cairo is the 2D, antialiased drawing library that GNOME
uses to draw things to the screen or to generate output for printing.

Do you want to render non-animated SVGs to a Cairo surface with a
minimal, no-nonsense API?  Librsvg may be adequate for you.

Using librsvg
-------------

**Documentation:** You can read the [documentation for librsvg][docs] at
developer.gnome.org.  Please [tell us][mail] if you don't find
something there that you need.

**Bug tracking:** If you have found a bug, take a look at [our bug
tracker][bugs].  Please see the "[reporting bugs][reporting-bugs]"
section in the file [CONTRIBUTING.md][contributing] to see how to
provide a good bug report.

**Asking questions:** Feel free to ask questions about using librsvg
in the [desktop-devel-list][d-d-l] mailing list.

**Programming languages:** There are bindings for librsvg in
programming languages other than C:  FIXME: include links to the
various bindings.

Contributing to librsvg's development
-------------------------------------

There is a code of conduct for contributors to librsvg; please see the
file [`code_of_conduct.md`][coc].

For information on how to report bugs, or how to contribute to librsvg
in general, please see the file `CONTRIBUTING.md`.

Goals of librsvg
----------------

Librsvg aims to be a low-footprint library for rendering SVG images.
It is used primarily in the [GNOME project](https://www.gnome.org) to render
SVG icons and vector images that appear on the desktop.  It is also
used in Wikimedia to render the SVG images that appear in Wikipedia,
so that even old web browsers can display them.

We aim to be a "render this SVG for me, quickly, and with a minimal
API" kind of library.  The SVG specification is huge, and definitely
contains features that are not frequently used in the Real World, if
at all.

Feature additions will be considered on a case-by-case basis.  Extra
points if you provide a proof-of-concept patch, and an example of the
situation in which you encountered that missing feature!

Non-goals of librsvg
--------------------

We don't aim to:

* Implement every single SVG feature that is in the spec.

* Implement external access to the SVG's DOM.

* Implement support for CSS-based animations (but if you can think of
  a nice API to do this, we'd be glad to know!)

* Replace the industrial-strength SVG rendering machinery in modern
  web browsers.

Of course, [contributions are welcome][contributing].  In particular,
if you find nice ways of doing the above while still maintaining the
existing API of librsvg, we would love to know about it!

Maintainers
-----------

The maintainer of librsvg is [Federico Mena Quintero].  You can [mail
me][mail] for any other questions you have about librsvg.

[gnome]: https://www.gnome.org/
[cairo]: https://www.cairographics.org/
[coc]: code-of-conduct.md
[docs]: https://developer.gnome.org/rsvg/stable/
[mail]: mailto:federico@gnome.org
[bugs]: http://bugzilla.gnome.org/enter_bug.cgi?product=librsvg
[contributing]: CONTRIBUTING.md
[reporting-bugs]: CONTRIBUTING.md#reporting-bugs
[d-d-l]: https://mail.gnome.org/mailman/listinfo/desktop-devel-list
[federico]: https://people.gnome.org/~federico/
