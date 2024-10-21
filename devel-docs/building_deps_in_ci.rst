Building dependencies in CI
===========================

**Status as of 2024/Sep/11:** implemented in :pr:`896`.
Non-Rust dependencies are built from git tags and installed in the CI
container image.  See the ``ci/build-dependencies.sh`` script that
does this.

Preamble
--------

Until Sep/2023, librsvg's CI has worked by building a container image
out of a snapshot of openSUSE Tumbleweed, a rolling release, that has
all of librsvg's dependencies in it.  What this means for runtime
dependencies like Cairo, Pango, etc. is that they come in a
"reasonably recent" version, but they are not pinned, and can change
any time that the rolling release decides to update them.

This is not entirely terrible: librsvg's run-time dependencies are all
stable, mature libraries that don't change very much.  From librsvg's
viewpoint, the only trouble comes in situations like these:

* A library involved in text rendering changes something, and text
  output changes slightly.  The test suite breaks as a result, since
  it assumes exact rendering based on reference images.

* Less often, a library involved in Bézier path rendering changes
  something, and parts of the test suite break because antialiasing is
  not the same as before.

* Someone runs librsvg's test suite with a different set of dependency
  versions than the test suite assumes; a bunch of tests fail for
  them, and they file bugs that are not very useful.  In the best
  case, they are using newer libraries and the bug reports let me (the
  maintainer) know that I'll need to re-generate the test suite's
  images soon.  In the worst case, I just close those bugs since they
  don't provide useful information.

And probably the biggest problem of all: I am never sure of exactly
what set of versions are okay for the test suite to work; I just
regenerate test reference files when they break after a CI update.

Pinning dependency versions
---------------------------

Librsvg's CI needs to be able to build the library's dependencies in a
custom fashion, without necessarily assuming that the dependencies
come from system libraries.  If we have that ability, then we can do a
few interesting things:

* Pin the versions of dependencies to a particular set, for example,
  one that corresponds to a certain GNOME release.  This is important
  to keep CI working for old branches, so security patches are easy to
  build.

* Compile the dependencies with a particular set of compiler options.
  For example, for fuzzing, dependencies should be built with
  sanitizers like asan/ubsan.  For deep debugging, it would be nice to
  have all the dependencies built in debug mode.  For performance
  testing, compile all the dependencies in release mode, etc.

Which dependencies?  These ones; this list is already sorted in the
correct build order:

* glib
* gobject-introspection
* freetype2
* fontconfig
* cairo
* harfbuzz
* pango
* libxml2
* gdk-pixbuf

How do we achieve that?
-----------------------

**Option 1:** There is a script ``ci/build-dependencies.sh`` that can
already build librsvg's dependencies pinned to particular git tags.
In theory one can pass environment variables to change compiler
options; the script may need some tweaks to change the meson or
autotools invocations.  The script builds and installs the
dependencies to a given prefix, and assumes that
``PATH/LD_LIBRARY_PATH/PKG_CONFIG_PATH`` are tweaked to use things
from that prefix.

**Option 2:** Alternatively, we can outsource the problem and use
GNOME's BuildStream images.  These correspond to specific releases of
the GNOME platform libraries, including librsvg's dependencies... and
librsvg itself, as it *is* a platform library.  One must be a little
careful to keep the test suite from using the "system's" librsvg in
that case, but this is not a huge problem.

Implementation notes - building dependencies explicitly
-------------------------------------------------------

As of Sep/2023 the CI builds three main images:

* An image with the MSRV, just used to test the promise that the MSRV
  can still build the library and its Rust dependencies.

* An image with a recent, stable Rust compiler - the main image used
  for most jobs, and what I use for local development.

* An image with the nightly compiler.  This is not updated frequently
  since the whole CI doesn't regenerate its images nightly.

* (Other images for other architectures or distros, not in scope for
  this discussion.)

All images have whatever RPM versions are available in openSUSE for
librsvg's runtime dependencies.

Proposed change:

* Keep a single image, pre-populated with ``rustup toolchain install
  <version>`` for the MSRV, the stable compiler, and nightly.  Select
  the compiler version on a per-job basis; hopefully this will be fast
  since ``toolchain`` should cache them.

* In that single image, keep pre-built sets of runtime dependencies
  (e.g. libraries built from git tags) in at least two configurations:
  the minimum supported versions, and the latest stable ones.  These
  configurations can live in different prefixes, 
  e.g. ``/usr/local/librsvg-minimum`` and
  ``/usr/local/librsvg-stable``.

The idea is to ensure that the minimum-supported everything (rustc and
dependencies) actually works, in addition to testing the recent-stable
stuff.

Keeping everything in a single container image (versions of the Rust
toolchain, and sets of dependencies installed to different prefixes)
is just an optimization to build and maintain a single image, instead
of the three we have right now.  If we determine that selecting
rustc/deps makes jobs too slow, we can go back to producing multiple
images.

Implementation note - BuildStream to test nightly dependencies
--------------------------------------------------------------

FIXME: alatiera's work goes here.

Other architectures, other distros
----------------------------------

I'd like to keep an ``aarch64`` image; it has let us notice
peculiarities like the different signedness of ``libc::c_char``.  It
doesn't need the minimum/stable/nightly distinction; we can keep it
stable-only as currently.

I think we can drop the Fedora image.  It still runs Fedora 36, is
seldom updated, and I don't pay attention to it.  I'd rather make it
possible to have explicit git versions of dependencies.

