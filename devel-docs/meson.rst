Meson build system
==================

As of 2022/Oct, librsvg still uses autotools to call ``cargo`` and
build all the artifacts that require the C API of librsvg to be built
beforehand.

The idea is to switch to meson and cargo-cbuild at the same time.
It's not really worth tweaking the autotools files to deal with
cargo-cbuild just to remove libtool; it is better to switch to a
friendlier build system that the rest of GNOME prefers already.

Librsvg's artifacts are listed in :doc:`product` - a reminder, with a
few extra intermediate stages:

- ``configure.ac``, to be replaced with ``meson.build``, has the
  version number.  ``build.rs`` looks for it there using a regex, and
  generates Rust code that includes the version number.  It also
  builds ``rsvg-version.h`` for the benefit of the C API artifacts,
  but it may be better to generate it from ``meson.build`` to make
  meson happier.

- The Rust code can be used to build three artifacts: the static
  library ``librsvg.a``, the dynamic library ``librsvg.so``, and the
  ``rsvg-convert`` executable.

- The header files are static / written by hand, with the exception of
  ``rsvg-version.h``, which is generated.  As noted above, meson can
  directly generate that file instead of going through ``build.rs``.

- From ``librsvg.so`` and the header files, build the ``.gir`` and
  ``.typelib`` introspection data.

- From the ``.gir``, build Vala bindings.

- From the ``.gir`` and headers, build the C API docs with ``gi-docgen``.

- The ``.pc`` file for pkg-config can be built by ``cargo-cbuild``.

- Building the ``.man`` page for ``rsvg-convert`` doesn't depend on
  anything but ``rst2man``.

- The source tree already contains an experimental port of the
  gdk-pixbuf loader to Rust.  We can either build that trivially with
  ``cargo`` and install it with Meson, or keep building the C version
  of the gdk-pixbuf loader with Meson.  I'd like to remove the C code
  and see what the rest of the platform thinks of having a big binary
  for a gdk-pixbuf loader.

- Meson also needs to be able to ``cargo test`` as part of the test
  suite, although that does not generate installable artifacts.
