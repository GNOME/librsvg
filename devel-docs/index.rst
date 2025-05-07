Development guide for librsvg
=============================

.. toctree::
   :caption: For Distributors and End Users
   :maxdepth: 1
   :hidden:

   supported_versions
   product
   features
   roadmap
   compiling
   security
   bugs

.. toctree::
   :caption: Getting Started as a Contributor
   :maxdepth: 1
   :hidden:

   devel_environment
   contributing
   learning

.. toctree::
   :caption: Understand the Code
   :maxdepth: 1
   :hidden:

   Documentation of the library's internals <https://gnome.pages.gitlab.gnome.org/librsvg/internals/rsvg/index.html>
   architecture
   adding_a_property
   memory_leaks

.. toctree::
   :caption: Design Documents
   :maxdepth: 1
   :hidden:

   text_layout
   render_tree
   api_observability
   performance_tracking
   viewport
   custom_properties
   building_deps_in_ci
   xml_parser

.. toctree::
   :caption: Info for Maintainers
   :maxdepth: 1
   :hidden:

   releasing
   ci
   oss_fuzz

.. toctree::
   :caption: Tools
   :maxdepth: 1
   :hidden:

   rsvg_bench

Welcome to the developer's guide for librsvg.  This is for people who
want to work on the development of librsvg itself, not for users of
the library or the ``rsvg-convert`` program.

If you want to modify this document, please see :source:`its source code
<devel-docs>` and :doc:`this guide <devel_docs_mod_guide>`.

Introduction
------------

Librsvg is a project with a long history; it started 2001 as a way to
use the then-new Scalable Vector Graphics format (SVG) for GNOME's
icons and other graphical assets on the desktop.  Since then, it has
evolved into a few different tools.

- :doc:`supported_versions` - Versions where you can expect support and bugfixes.
- :doc:`product` - What comes out of this repository once it is compiled?
- :doc:`features` - Supported elements, attributes, and properties.
- :doc:`roadmap` - Ever-changing list of priorities for the
  maintainers; check this often!
- :doc:`compiling` - Cross compilation, debug/release builds, special options.
- :doc:`security` - Reporting security bugs, releases with security
  fixes, security of dependencies.
- :doc:`bugs`

Getting started
---------------

- :doc:`devel_environment`
- :doc:`contributing`
- :doc:`learning`

Understand the code
-------------------

.. Test suite - move tests/readme here?

- `Documentation of the library's internals
  <https://gnome.pages.gitlab.gnome.org/librsvg/internals/rsvg/index.html>`_
- :doc:`architecture`
- :doc:`adding_a_property`
- :doc:`memory_leaks`

Design documents
----------------

Before embarking on big changes to librsvg, please write a little
design document modeled on the following ones, and submit a merge
request.  We can then discuss it before coding.  This way we will have
a sort of big-picture development history apart from commit messages.

- :doc:`text_layout`
- :doc:`render_tree`
- :doc:`api_observability`
- :doc:`performance_tracking`
- :doc:`viewport`
- :doc:`custom_properties`
- :doc:`building_deps_in_ci`

See https://rustc-dev-guide.rust-lang.org/walkthrough.html, section
Overview, to formalize the RFC process for features vs. drive-by
contributions.

Information for maintainers
---------------------------

- :doc:`releasing`
- :doc:`ci`
- :doc:`oss_fuzz`

..
   - Overview of the maintainer's workflow.
   - Marge-bot.
   - Documentation on the CI.
   - Documentation on the OSS-Fuzz integration and its maintenance.

Tools
-----

- :doc:`rsvg_bench`

References
----------

- `SVG2 specification <https://www.w3.org/TR/SVG2/>`_.  This is the current Candidate Recommendation and it should
  be your main reference...

- ... except for things which are later clarified in the `SVG2 Editor's Draft <https://svgwg.org/svg2-draft/>`_.

- `Filter Effects Module Level 1 <https://www.w3.org/TR/filter-effects/>`_.

- `References listed in the SVG2 spec
  <https://www.w3.org/TR/SVG2/refs.html>`_ - if you need to consult
  the CSS specifications.
  
- `SVG1.1 specification <https://www.w3.org/TR/SVG11/>`_.  Use this mostly for historical reference.

- `SVG Working Group repository
  <https://github.com/w3c/svgwg/tree/master>`_.  The github issues are
  especially interesting.  Use this to ask for clarifications of the
  spec.

- `SVG Working Group page <https://svgwg.org/>`_.

- Presentation at GUADEC 2017, `Replacing C library code with Rust: What I learned with
  librsvg <https://viruta.org/docs/fmq-porting-c-to-rust.pdf>`_.    It gives
  a little history of librsvg, and how/why it was being ported to Rust
  from C.

- Presentation at GUADEC 2018, `Patterns of refactoring C to Rust: the case of
  librsvg <https://viruta.org/docs/fmq-refactoring-c-to-rust.pdf>`_.  It
  describes ways in which librsvg's C code was refactored to allow
  porting it to Rust.

- `Federico Mena's blog posts on librsvg
  <https://viruta.org/tag/librsvg.html>`_ - plenty of of history and
  stories from the development process.

Talks on librsvg.
