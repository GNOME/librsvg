Librsvg as a product
====================

A full build of librsvg produces several *artifacts*, which are the
final "products" that are produced from the build.  We will discuss
them in three groups: library artifacts, ``rsvg-convert`` artifacts,
and others.


Library artifacts
-----------------

Librsvg is part of the `GNOME platform libraries
<https://developer.gnome.org/documentation/introduction/overview/libraries.html>`_,
and needs to maintain a C-compatible API with ABI stability across versions.

The build produces these artifacts, which are typical of GNOME libraries that
can be used from C and other languages:

- A shared library ``librsvg-2.so`` (the file extension will be
  different on MacOS or Windows).  This is usually installed as part
  of the system's libraries.  For example, the GTK toolkit assumes
  that librsvg's library is installed in the system, and uses it to
  load SVG assets like icons.  Applications can generally link to this
  library and load SVG documents for different purposes.

- A group of C header files (``*.h``) that will be installed in the
  system's location for header files.  C and C++ programs can use
  these directly.

- A ``librsvg-2.pc`` file for `pkg-config
  <https://www.freedesktop.org/wiki/Software/pkg-config/>`_.  This lets
  compilation scripts find the location of the installed library and
  header files.

- ``.gir`` and ``.typelib`` files for `GObject Introspection
  <https://gi.readthedocs.io/en/latest/>`_.  These are machine-readable
  descriptions of the API/ABI in the ``.so`` library, which are used by
  language bindings to make librsvg's functionality available to many
  programming languages.

- A ``.vapi`` description of the API for the `Vala language
  <https://vala.dev/>`_ compiler.

Rust API
^^^^^^^^

Apart from the C-compatible library, the Rust code for the library
defines a ``librsvg`` crate that can be used by Rust programs.  Since
version 2.57.0, librsvg is available as a regular crate in
``crates.io``.


``rsvg-convert`` artifacts
--------------------------

``rsvg-convert`` is a command-line tool to render SVG documents to
various output formats.  It is a very widely-used tool, and many
scripts and systems depend on it maintaining a stable set of
command-line options.

The build produces these:

- The ``rsvg-convert`` executable.  This is the tool that most
  end-users interact with.

- A Unix manual page for ``rsvg-convert(1)``.


Other artifacts
---------------

- A ``libpixbufloader-svg.so`` module for `gdk-pixbuf
  <https://docs.gtk.org/gdk-pixbuf/>`_.  This allows programs to use
  the gdk-pixbuf API to load SVG documents, as if they were raster
  files like JPEG or PNG.

- A ``librsvg.thumbnailer`` configuration file, to tell GNOME's
  thumbnailing mechanism that it can just use gdk-pixbuf when trying
  to create a thumbnail for an SVG file.  These thumbnails can then
  get displayed in file managers.

- `Documentation for the C API
  <https://gnome.pages.gitlab.gnome.org/librsvg/Rsvg-2.0/index.html>`_,
  published online and also installed on the system in a place where
  GNOME's `DevHelp <https://gitlab.gnome.org/GNOME/devhelp>`_ can find
  it.

- `Documentation for the Rust API
  <https://gnome.pages.gitlab.gnome.org/librsvg/doc/rsvg/index.html>`_,
  published online.  This is not built from the normal ``make`` process,
  but independently as part of the :doc:`ci` pipeline.

- The rendered HTML version of this development guide.  This is not
  built from the normal ``make`` process, but independently as part of
  the :doc:`ci` pipeline.
