..
  Please also check to see if OSS-Fuzz dependencies need to be changed (see oss_fuzz.rst).

To compile librsvg, you need the following packages installed.  The
minimum version is listed here; you may use a newer version instead.

**Compilers and build tools:**

* a C compiler
* `rust <https://www.rust-lang.org/>`_ 1.85.1 or later
* `cargo <https://www.rust-lang.org/>`_
* ``cargo-cbuild`` from `cargo-c <https://github.com/lu-zero/cargo-c>`_
* `meson <https://mesonbuild.com/>`_
* `vala <https://vala.dev/>`_ (optional)

**Mandatory dependencies:**

* `Cairo <https://gitlab.freedesktop.org/cairo/cairo>`_ 1.18.0 with PNG support
* `Freetype2 <https://gitlab.freedesktop.org/freetype/freetype>`_ 2.8.0
* `GLib <https://gitlab.gnome.org/GNOME/glib/>`_ 2.50.0
* `Libxml2 <https://gitlab.gnome.org/GNOME/libxml2>`_ 2.9.0
* `Pango <https://gitlab.gnome.org/GNOME/pango/>`_ 1.50.0

**Optional dependencies:**

* `GDK-Pixbuf <https://gitlab.gnome.org/GNOME/gdk-pixbuf/>`__ 2.20.0
* `GObject-Introspection <https://gitlab.gnome.org/GNOME/gobject-introspection>`_ 0.10.8
* `gi-docgen <https://gitlab.gnome.org/GNOME/gi-docgen>`_
* `python3-docutils <https://pypi.org/project/docutils/>`_
* `dav1d <https://code.videolan.org/videolan/dav1d>`_ 1.3.0 (to support the AVIF image format)
