Detailed compilation instructions
=================================

A full build of librsvg requires the
`meson build system <https://mesonbuild.com>`_. A full build will
produce these artifacts (see :doc:`product` for details):

-  ``rsvg-convert`` binary and its ``man`` page.
-  librsvg shared library with the GObject-based API.
-  GDK-Pixbuf loader for SVG files.
-  HTML documentation for the GObject-based API, with ``gi-docgen``.
-  GObject-introspection information for language bindings.
-  Vala language bindings.

Some of the artifacts above are optional; please see the section
:ref:`compile_time_options` below for details.

It is perfectly fine to :source:`ask the maintainer <README.md#maintainers>`
if you have questions about the meson setup; it’s a tricky bit of
machinery, and we are glad to help.

The rest of this document explains librsvg’s peculiarities apart from
the usual way of compiling meson projects.

.. _build_time_dependencies:

Build-time dependencies
-----------------------

.. include:: _build_dependencies.rst

See :doc:`devel_environment` for details on how to install these dependencies.

.. _basic_compilation_instructions:

Basic compilation instructions
------------------------------

If you are compiling a tarball:

.. code:: sh

   mkdir -p _build
   meson setup _build -Ddocs=enabled -Dintrospection=enabled -Dvala=enabled
   meson compile -C _build
   meson install -C _build

The options that start with ``-D`` are listed in the
``meson_options.txt`` file and are described in the next section.

.. _compile_time_options:

Compile-time options
--------------------

These are invoked during ``meson setup`` as ``-Doption_name=value``.
See `meson's documentation on using build-time options
<https://mesonbuild.com/Build-options.html>`_ for details.

These are librsvg's options:

* ``introspection`` - Specifies whether the build will generate
  `GObject Introspection <https://gi.readthedocs.io/en/latest/>`_
  information for language bindings.  Values are
  ``enabled``/``disabled``/``auto``.

* ``pixbuf`` - Specifies whether to build with support for `gdk-pixbuf
  <https://docs.gtk.org/gdk-pixbuf/>`_ in the library APIs.
  Values are ``enabled``/``disabled``/``auto``.

* ``pixbuf-loader`` - Specifies whether to build a `gdk-pixbuf
  <https://docs.gtk.org/gdk-pixbuf/>`_ module to let applications which use
  gdk-pixbuf load and render SVG files as if they were raster images.
  Values are ``enabled``/``disabled``/``auto``.

* ``rsvg-convert`` - Specifies whether to build the `rsvg-convert`
  binary.  You can disable this if you just need the library to link
  into other programs.  Values are ``enabled``/``disabled``/``auto``.

* ``docs`` - Specifies whether the C API reference and the
  rsvg-convert manual page should be built.  These require ``gi-docgen
  <https://gnome.pages.gitlab.gnome.org/gi-docgen/>`_ and ``rst2man``
  from Python's `docutils <https://www.docutils.org/>`_, respectively.
  Values are ``enabled``/``disabled``/``auto``.

* ``vala`` - Specifies whether a `Vala <https://vala.dev/>`_ language
  binding should be built.  Requires the Vala compiler to be
  installed.  Values are ``enabled``/``disabled``/``auto``.

* ``tests`` - Specifies whether the test suite should be built.
  Value is a boolean that defaults to ``true``.

* ``triplet`` - Specifies the `Rust target triplet
  <https://doc.rust-lang.org/stable/rustc/platform-support.html>`_; 
  only needed for cross-compilation.  Value is a string.

* ``avif`` - Specifies whether the image-rs crate, which librsvg uses
  to load raster images, should be built with support for the AVIF
  format.  Requires the `dav1d
  <https://code.videolan.org/videolan/dav1d>`_ library.  Values are
  ``enabled``/``disabled``/``auto``.

* ``rustc-version`` - Specifies the ``rustc`` version to use; only
  supported on Windows.  Value is a string.


.. _building_with_no_network_access:

Building with no network access
-------------------------------

Automated build systems generally avoid network access so that they can
compile from known-good sources, instead of pulling random updates from
the net every time. However, normally Cargo likes to download
dependencies when it first compiles a Rust project.

You can use `cargo vendor
<https://doc.rust-lang.org/cargo/commands/cargo-vendor.html>`_ to
download librsvg's Rust dependencies ahead of time, so that subsequent
compilation don't require network access.

Build systems can use `Cargo’s source replacement
mechanism <https://doc.rust-lang.org/cargo/reference/source-replacement.html>`_ to override
the location of the source code for the Rust dependencies, for example,
in order to patch one of the Rust crates that librsvg uses internally.
