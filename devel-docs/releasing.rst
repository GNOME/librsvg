Release process checklist
=========================

Feel free to print this document or copy it to a text editor to check
off items while making a release.

- ☐ Refresh your memory with
  https://wiki.gnome.org/MaintainersCorner/Releasing

**Versions:**

- ☐ Increase the package version number in ``meson.build`` (it may
  already be increased but not released; double-check it).
- ☐ Copy version number to ``Cargo.toml``.
- ☐ Copy version number to ``doc/librsvg.toml``.
- ☐ Compute crate version number and write it to ``rsvg/Cargo.toml``, see :ref:`crate version<crate_version>` below.
- ☐ Copy the crate version number to the example in ``rsvg/src/lib.rs``.
- ☐ ``cargo update -p librsvg`` - needed because you tweaked ``Cargo.toml``, and
  also to get new dependencies.
- ☐ Tweak the library version number in ``meson.build`` if the API
  changed; follow the steps there.
- ☐ Adjust the supported versions in ``README.md``.
- ☐ Adjust the supported versions in ``devel-docs/supported_versions.rst``.
- ☐ Adjust the supported versions in ``.gitlab/issue_templates/default.md``.

**Rust Bindings:**

- ☐ Make sure that librsvg-rebind is in sync with librsvg C bindings by calling ``./librsvg-rebind/regen.sh``
- ☐ If the bindings have changed from the last version, increase the package version in
   - ☐ librsvg-rebind/librsvg-rebind/Cargo.toml
   - ☐ librsvg-rebind/librsvg-rebind/sys/Cargo.toml

**Release notes:**

- ☐ Update ``NEWS``, see below for the preferred format.

**CI:**

- ☐ Commit the changes above; push a branch.
- ☐ Create a merge request; fix it until it passes the CI.  Merge it.

**Publish:**

- ☐ Publish ``librsvg`` to crates.io, see :ref:`crate_release` for details.
   - ☐ ``cargo publish -p librsvg``

- ☐ Publish ``librsvg-rebind`` to crates.io:
   - The publish process does a test compilation and needs the `.so` installed to build, so do
     ``meson setup _build --prefix /usr/local/librsvg && meson compile -C _build && meson install -C _build``
   - ☐ ``cargo publish -p librsvg-rebind-sys``
   - ☐ ``cargo publish -p librsvg-rebind``
- ☐ If this is a development release, create a signed tag for the crate's version - ``git tag -s x.y.z-beta.w``.
- ☐ Create a signed tag for the merge commit - ``git tag -s x.y.z`` with the version number.
- ☐ If this is a development release ``git push`` the signed tag for the crate's version to gitlab.gnome.org/GNOME/librsvg
- ☐ ``git push`` the signed tag for the GNOME version to gitlab.gnome.org/GNOME/librsvg
- ☐ Optionally edit the `release page ing Gitlab <https://gitlab.gnome.org/GNOME/librsvg/-/releases>`_.

For ``x.y.0`` releases, do the following:

-  ☐ `Notify the release
   team <https://gitlab.gnome.org/GNOME/releng/-/issues>`__ on whether
   to use this ``librsvg-x.y.0`` for the next GNOME version via an issue
   on their ``GNOME/releng`` project.

-  ☐ ``cargo-audit audit`` and ensure we don’t have vulnerable
   dependencies.

Gitlab release
--------------

-  ☐ Select the tag ``x.y.z`` you just pushed.

-  ☐ If there is an associated milestone, select it too.

-  ☐ Fill in the release title - ``x.y.z``.

-  ☐ Copy the release notes from NEWS (By default it uses the GIT_TAG_MESSAGE).

-  ☐ Add a release asset link to
   ``https://download.gnome.org/sources/librsvg/x.y/librsvg-x.y.z.tar.xz``
   and call it ``librsvg-x.y.z.tar.xz - release tarball``.

-  ☐ Add a release asset link to
   ``https://download.gnome.org/sources/librsvg/x.y/librsvg-x.y.z.sha256sum``
   and call it
   ``librsvg-x.y.z.sha256sum - release tarball       sha256sum``.

Version numbers and release schedule
------------------------------------

``meson.build`` and ``Cargo.toml`` must have the same **package
version** number - this is the number that users of the library see.

``meson.build`` is where the **library version** is defined; this is
what gets encoded in the SONAME of ``librsvg.so``.

Librsvg follows `GNOME's release versioning as of 2022/September
<https://discourse.gnome.org/t/even-odd-versioning-is-confusing-lets-stop-doing-it/10391>`_.
(Note that it used an even/odd numbering scheme before librsvg 2.55.x)

Librsvg follows `GNOME's six-month release schedule
<https://wiki.gnome.org/ReleasePlanning>`_.

The `release-team <https://gitlab.gnome.org/GNOME/releng/-/issues>`__
needs to be notified when a new series comes about, so they can adjust
their tooling for the stable GNOME releases. File an
issue in their `repository
<https://gitlab.gnome.org/GNOME/releng/-/issues>`__ to indicate that
the new ``librsvg-x.y.0`` is a stable series.

.. _crate_version:

Version number for public Rust crate
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The ``librsvg`` crate is `available on crates.io
<https://crates.io/crates/librsvg/>`_.  This is for people who wish to
use librsvg directly from Rust, instead of via the C ABI library
(i.e. the ``.tar.xz`` release).

While the C ABI library uses the GNOME versioning scheme, Rust crates
use `SemVer <https://semver.org>`_.  So, for librsvg, we have the
following scheme:

**Stable releases:**

* GNOME tarball: 2.57.0
* Rust crate: 2.57.0 (i.e. the same)

**Development releases:**

* GNOME tarball: 2.57.90 through 2.57.99 (.9x patch version means development release)
* Rust crate: 2.58.0-beta.0 through -beta.9 (SemVer supports a -beta.x suffix)

When making releases, you have to edit ``Cargo.toml`` and
``rsvg/Cargo.toml`` by hand to put in version numbers like the above.
The CI scripts will check that the correct versions are in place.

.. _crate_release:

Releasing to crates.io
----------------------

After preparing a GNOME release, you'll also want to release to
crates.io.  This requires an `API token
<https://doc.rust-lang.org/cargo/reference/publishing.html#before-your-first-publish>`_;
if you are maintainer you should have one, and also write access to
the ``librsvg`` crate on crates.io.

To make a release, ``cargo publish -p librsvg``.

To publish the Rust bindings to the C library, ``cargo publish -p librsvg-rebind-sys``, ``cargo publish -p librsvg-rebind``.

After this succeeds, proceed with the rest of the steps in the
ref:`release_process_checklist`.

Minimum supported Rust version (MSRV)
-------------------------------------

While it may seem desirable to always require the latest released
version of the Rust toolchain, to get new language features and such,
this is really inconvenient for distributors of librsvg which do not
update Rust all the time. So, we make a compromise.

The ``meson.build`` script defines ``msrv`` with librsvg’s minimum
supported Rust version (MSRV).  This ensures that distros will get an
early failure during a build, at the ``meson setup`` step, if they have
a version of Rust that is too old — instead of getting an obscure
error message from ``rustc`` in the middle of the build when it finds
an unsupported language construct.

Please update all of these values when increasing the MSRV:

- ``msrv`` in ``meson.build``.

- ``cargo_c`` version in ``meson.build``.

- ``rust-version`` in ``Cargo.toml``.

- ``RUST_MINIMUM`` in ``ci/container_builds.yml``.

- The ``Compilers and build tools`` section in ``devel-docs/_build_dependencies.rst``.

Sometimes librsvg’s dependencies update their MSRV and librsvg may need
to increase it as well. Please consider the following before doing this:

-  Absolutely do not require a nightly snapshot of the compiler, or
   crates that only build on nightly.

-  Distributions with rolling releases usually keep their Rust
   toolchains fairly well updated, maybe not always at the latest, but
   within two or three releases earlier than the latest. If the MSRV you
   want is within about six months of the latest, things are probably
   safe.

-  Enterprise distributions update more slowly. It is useful to watch
   for the MSRV that Firefox requires, although sometimes Firefox
   updates Rust very slowly as well. Now that distributions are shipping
   packages other than Firefox that require Rust, they will probably
   start updating more frequently.

Generally — two or three releases earlier than the latest stable Rust is
OK for rolling distros, probably perilous for enterprise distros.
Releases within a year of an enterprise distro’s shipping date are
probably OK.

If you are not sure, ask on the `forum for GNOME
distributors <https://discourse.gnome.org/tag/distributor>`__ about
their plans! (That is, posts on ``discourse.gnome.org`` with the
``distributor`` tag.)

Format for release notes in NEWS
--------------------------------

The ``NEWS`` file contains the release notes. Please use something
close to this format; it is not mandatory, but makes the formatting
consistent, and is what tooling expects elsewhere - also by writing
Markdown, you can just cut&paste it into a Gitlab release. You can skim
bits of the ``NEWS`` file for examples on style and content.

New entries go at the **top** of the file.

::

   Version x.y.z
   =============

   Commentary on the release; put anything here that you want to
   highlight.  Note changes in the build process, if any, or any other
   things that may trip up distributors.

   ## Description of a special feature

   You can include headings with `##` in Markdown syntax.

   Blah blah blah.


   Next is a list of features added and issues fixed; use gitlab's issue
   numbers. I tend to use this order: first security bugs, then new
   features and user-visible changes, finally regular bugs.  The
   rationale is that if people stop reading early, at least they will
   have seen the most important stuff first.

   ## Changes:

   - #123 - title of the issue, or short summary if it warrants more
     discussion than just the title.

   - #456 - fix blah blah (Contributor's Name).

   ## Special thanks for this release:

   - Any people that you want to highlight.  Feel free to omit this
     section if the release is otherwise unremarkable.

Making a tarball
----------------

Don't make a tarball by hand.  Let the CI system do it.  Look for
``distcheck`` in the checklist above.  That job in the CI pipelines
has the release tarball which you can download.

Copying the tarball to master.gnome.org
---------------------------------------

If you don’t have a maintainer account there, ask federico@gnome.org to
do it or `ask the release
team <https://gitlab.gnome.org/GNOME/releng/-/issues>`__ to do it by
filing an issue on their ``GNOME/releng`` project.

Rust dependencies
-----------------

Librsvg's ``Cargo.lock`` is checked into git because the resolved
versions of crates that it mentions are the ones that were actually
used to run the test suite automatically in CI, and are "known good".
In other words: `keep the results of dependency resolution in version
control, and update those results manually
<https://blog.ometer.com/2017/01/10/dear-package-managers-dependency-resolution-results-should-be-in-version-control/>`_.

It is important to keep these dependencies updated; you can do that
regularly with the ``cargo update`` step listed in the checklist
above.

`cargo-audit <https://github.com/rustsec/rustsec>`__ is very useful to
scan the list of dependencies for registered vulnerabilities in the
`RustSec vulnerability database <https://rustsec.org/>`__. Run it
especially before making a new ``x.y.0`` release, or check the output
of the ``deny`` job in CI pipelines — this runs `cargo-deny
<https://embarkstudios.github.io/cargo-deny/>`_ to check for
vulnerable and duplicate dependencies.

Sometimes cargo-audit will report crates that are not vulnerable, but
that are unmaintained. Keep an eye of those; you may want to file bugs
upstream to see if the crates are really unmaintained or if they should
be substituted for something else.

Creating a stable release branch
--------------------------------

-  Create a branch named ``librsvg-xx.yy``, e.g. ``librsvg-2.54``

-  Make the ``BASE_TAG`` in ``ci/container-builds.yml`` refer to the new
   ``librsvg-xx.yy`` branch instead of ``main``.

-  Push that branch to origin.

-  (Branches with that naming scheme are already automatically protected
   in gitlab’s Settings/Repository/Protected branches.)

-  Edit the badge for the stable branch so it points to the new branch:
   Settings/General/Badges, find the existing badge for the stable
   branch, click on the edit button that looks like a pencil. Change the
   **Link** and **Badge image URL**; usually it is enough to just change
   the version number in both.
