Librsvg as a product
====================

A full build of librsvg produces several *artifacts*.  For this
discussion, we will talk about the "library part" of the build, and
the "rsvg-convert" part.

The "library part" of the build produces the following:

- A shared library ``librsvg-2.so``.  This is usually installed as
  part of the system's libraries.  For example, the GTK toolkit
  assumes that librsvg's library is installed in the system, and uses
  it to load SVG assets like icons.

- A group of C header files (``*.h``) that will be installed in the
  system's location for header files.  C and C++ programs can use
  these directly.

- A ``librsvg-2.pc`` file for `pkg-config
  <https://www.freedesktop.org/wiki/Software/pkg-config/>`.  This lets
  compilation scripts find the location of the installed library and
  header files.

- ``.gir`` and ``.typelib`` files for `GObject Introspection
  <https://gi.readthedocs.io/en/latest/>`.  These are machine-readable
  descriptions of the API/ABI in the `.so` library, which are used by
  language bindings to make librsvg's functionality available to many
  programming languages.

- A ``.vapi`` description of the API for the `Vala <https://vala.dev/>` compiler.

