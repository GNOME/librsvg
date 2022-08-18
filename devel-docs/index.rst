Development guide for librsvg
=============================

.. toctree::
   product
   devel_environment
   contributing
   ci
   :maxdepth: 1
   :caption: Contents:

Welcome to the developer's guide for librsvg.  This is for people who
want to work on the development of librsvg itself, not for users of
the library or the `rsvg-convert` program.

If you want to modify this document, `please see its source code
<https://gitlab.gnome.org/GNOME/librsvg/-/tree/main/devel-docs>`_.

Introduction
------------

Librsvg is a project with a long history; it started 2001 as a way to
use the then-new Scalable Vector Graphics format (SVG) for GNOME's
icons and other graphical assets on the desktop.  Since then, it has
evolved into a few different tools.

You may want to familiarize yourself with :doc:`product` to read about
these tools, and the things that a full build produces: the library
itself, the ``rsvg-convert`` executable, etc.

Getting started
---------------

- :doc:`devel_environment`

FIXME: link to doc with stuff from CONTRIBUTING.md's "Hacking on librsvg"

Add basic info on cloning the repo, getting a gitlab account, forking.

Development roadmap.

Understand the code
-------------------

FIXME: Overview of the source tree.

Tour of the code - load a file, render it.

Test suite - move tests/readme here?

Link to the internals documentation.

Design documents
----------------

Before embarking on big changes to librsvg, please write a little
design document modeled on the following ones, and submit a merge
request.  We can then discuss it before coding.  This way we will have
a sort of big-picture development history apart from commit messages.

See https://rustc-dev-guide.rust-lang.org/walkthrough.html, section
Overview, to formalize the RFC process for features vs. drive-by
contributions.

FIXME: link the md here.

Information for maintainers
---------------------------

FIXME: Move RELEASING.md here

Overview of the maintainer's workflow.

Marge-bot.

Documentation on the CI.

References
----------

Link to SVG/CSS specs; other useful bits.

Links to Mozilla's SVG, WebKit, resvg, Inkscape.

Talks on librsvg.

Indices and tables
------------------

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
