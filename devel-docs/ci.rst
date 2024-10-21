Continuous Integration
======================

Or, when robots are eager to help.

Librsvg's repository on gitlab.gnome.org is configured to use a
Continuous Integration (CI) pipeline, so that it compiles the code and
runs the test suite after every ``git push``.

If you have never read it before, please read `The Not Rocket Science
Rule of Software Engineering
<https://graydon2.dreamwidth.org/1597.html>`_, about automatically
maintaining a repository of code that always passes all the tests.
This is what librsvg tries to do!

In addition to running the test suite, the CI pipeline does other cool
things.  The pipeline is divided into *stages*.  Here is roughly what
they do:

- First, set up a *reproducible environment* to build and test things:
  this builds a couple of container images and automatically updates
  them in gitlab.  The container images have all of librsvg's
  dependencies, and all the tools required for compilation, building
  the documentation, and casual debugging.

- Then, run a quick ``cargo check`` ("does this have a chance of
  compiling?"), and ``cargo test`` (run a fast subset of the test
  suite).  This stage is intended to catch breakage early.

- Then, run the full test suite in a couple different configurations:
  different versions of Rust, different distros with slightly
  different versions of dependencies.  This stage is intended to catch
  common sources of breakage across environments.

- In parallel with the above, run ``cargo clippy`` and ``cargo fmt``.
  The first usually has good suggestions to improve the code; the latter
  is to guarantee a consistent indentation style.

- In parallel, obtain a test coverage report.  We'll talk about this below.

- Check whether making a release at this point would actually work:
  this builds a release tarball and tries to compile it and run the
  test suite again.

- Finally, generate documentation: reference docs for the C and Rust
  APIs, and the rendered version of this development guide.  Publish
  the docs and coverage report to a web page.

We'll explain each stage in detail next.

Creating a reproducible environment
-----------------------------------

The task of setting up CI for a particular distro or build
configuration is rather repetitive.  One has to start with a "bare"
distro image, then install the build-time dependencies that your
project requires, then that is slow, then you want to build a
container image instead of installing packages every time, then you
want to test another distro, then you want to make those container
images easily available to your project's forks, and then you start
pulling your hair.

`Fredesktop CI Templates
<https://gitlab.freedesktop.org/freedesktop/ci-templates/>`_
(`documentation
<https://freedesktop.pages.freedesktop.org/ci-templates/>`_) are a
solution to this.  They can automatically build container images for
various distros, make them available to forks of your project, and
have some nice amenities to reduce the maintenance burden.

Librsvg uses CI templates to test its various build configurations.
The container images are stored here:
https://gitlab.gnome.org/GNOME/librsvg/container_registry

See the section below on the "Full test suite and different
environments" for details on what gets tested on the different
container images produced by this stage.

.. NOTE: The target below is used outside this development guide.

.. _container-image-version:

.. important::

   Whenever changes are made to the CI environments (such as updating
   dependencies or CI tools, or their versions), the container image
   version tag defined as ``BASE_TAG`` in :source:`ci/container_builds.yml`
   should be incremented appropriately.

   The tag name is (by convention) of the format
  
   .. code::

      <date>.<version>-[<user_name>_]<branch_name>

   where:

   - *date* is the current date when incrementing the version tag and is
     of the format ``YYYY-MM-DD``.
   - *version* is an index number (starting from zero) to differentiate
     images built on the same day for the same branch.
   - *branch_name* is the name of the branch on which the CI changes are
     being made.
   - *user_name* is the user name of the branch's owner and is optional
     but recommended for branches on forks, in order to avoid tag name
     clashes with equally-named branches on other forks.

   For example:

   - ``2024-10-20.0-main`` ->
     first iteration for ``GNOME/librsvg:main`` on 2024-10-20
   - ``2025-09-01.1-foo_bar`` ->
     second iteration for ``GNOME/librsvg:foo-bar`` on 2025-09-01
   - ``2099-12-31.3-federico_bar_foo`` ->
     third iteration for ``federico/librsvg:bar-foo`` on 2099-12-31

   For any branch that is **not** :source:`GNOME/librsvg:main <main:>` and
   is intended **to be merged** into it: Once the new container images are
   confirmed to be working as expected, the version tag should be
   incremented again **before merging** into ``main``, this time without
   *user_name* and with ``main`` as *branch_name*.
   This is always best done **when the branch is ready to be merged**, in
   order to avoid unnecessary tag name conflicts and CI surprises.
   In the case of conflicting tag names with ``main``, be sure to increment
   the *version* number if the tag name on ``main`` has the current date.
  
   If unsure whether a change you made is concerned or for any further
   clarification, please ask the
   :source:`maintainers <README.md#maintainers>`.


Quick checks
------------

``cargo check`` and ``cargo test`` run relatively quickly, and can catch
trivial compilation problems as well as breakage in the "fast" section
of the test suite.  When trying out things in a branch or a merge
request, you can generally look at only these two jobs for a fast
feedback loop.


Full test suite and different environments
------------------------------------------

- The "full test suite" in principle runs ``meson test``.
  This runs the "fast" portion of the test suite, but also a few slow
  tests which are designed to test librsvg's built-in limits.  It also
  runs the C API tests, which require a C compiler.

- There are builds use a certain Minimum Supported Rust Version
  (MSRV), also a relatively recent stable Rust, and Rust nightly.
  Building with the MSRV is to help distros that don't update Rust
  super regularly, and also to ensure that librsvg's dependencies do
  not suddently start depending on a too-recent Rust version, for
  example.  Building on nightly is hopefully to catch compiler bugs
  early, or to get an early warning when the Rust compiler is about to
  introduce newer lints/warnings.

- Build on a couple of distros.  Librsvg's test suite is especially
  sensitive to changes in rendering from Cairo, Pixman, and the
  Pango/Freetype2/Harfbuzz stack.  Building on a few distros gives us
  slightly different versions of those dependencies, so that we can
  catch breakage early.


Lints and formatting
--------------------

There is a job for ``cargo clippy``.  Clippy usually has very good
suggestions to improve the coding style, so take advantage of them!
And if Clippy's suggetions don't make sense for a particular portion
of the code, feel free to add exceptions like
``#[allow(clippy::foo_bar)]`` to the corresponding block.

There is a job for ``cargo fmt``.  Librsvg uses the default formatting
for Rust code.  For portions of code that are more legible if
indented/aligned by hand, please use ``#[rustfmt::skip]``.

One job runs ``cargo deny``, which checks if there are dependencies with
vulnerabilities.

Another job runs a script to check that the version numbers mentioned
in various parts of the source code all match.  For example,
``Cargo.toml`` and ``meson.build`` must have checks for the same Minimum
Supported Rust Version (MSRV).


Test coverage report
--------------------

There is a job that generates a `test coverage report
<https://gnome.pages.gitlab.gnome.org/librsvg/coverage/index.html>`_.
The code gets instrumented, and as the test suite runs, the
instrumentation remembers which lines of code were executed and which
ones were not; this then gets presented in an HTML report.  This can
be used for various things:

- See which parts of the code are not executed while running the test
  suite.  Maybe we need to add tests that cause them to run!

- If you disable most of the test suite, you can use the coverage
  report to explore which parts of the code get executed with a
  particular SVG.  This can aid in learning the code base.


Release tests
-------------

There is a job that runs ``meson dist``, a part of Meson that
simulates building a full release tarball.  Running this in the CI
helps us guarantee that librsvg is always in a release-worthy state.


Generate documentation
----------------------

The following sets of documentation get generated:

- `C API docs
  <https://gnome.pages.gitlab.gnome.org/librsvg/Rsvg-2.0/index.html>`_,
  with `gi-docgen <https://gitlab.gnome.org/GNOME/gi-docgen>`_.
- `Rust API docs <https://gnome.pages.gitlab.gnome.org/librsvg/doc/rsvg/index.html>`_, with ``cargo doc``.
- `Internals docs <https://gnome.pages.gitlab.gnome.org/librsvg/internals/rsvg/index.html>`_, with ``cargo doc --document-private-items``.
- `This development guide <https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/index.html>`_, with ``sphinx``.
  
