Continuous Integration - When robots are eager to help
======================================================

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

- In parallel with the above, run `cargo clippy` and `cargo fmt`.  The
  first usually has good suggestions to improve the code; the latter
  is to guarantee a consistent indentation style.

- In parallel, obtain a test coverage report.  We'll talk about this below.

- Check whether making a release at this point would actually work:
  this builds a release tarball and tries to compile it and run the
  test suite again.  This stage is intended to check that the
  autotools setup is up-to-date with respect to the git repository.

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

[Fredesktop CI Templates][ci-templates]
([documentation][ci-templates-docs]) are a solution to this.  They can
automatically build container images for various distros, make them
available to forks of your project, and have some nice amenities to
reduce the maintenance burden.

Librsvg uses CI templates to test its various build configurations.
The container images are stored here:
https://gitlab.gnome.org/GNOME/librsvg/container_registry

What sort of environments for building are produced by this step?

- Build with a certain Minimum Supported Rust Version (MSRV), also a
  relatively recent stable Rust, and Rust nightly.  Building with the
  MSRV is to help distros that don't update Rust super regularly, and
  also to ensure that librsvg's dependencies do not suddently start
  depending on a too-recent Rust version, for example.  Building on
  nightly is hopefully to catch compiler bugs early, or to get an
  early warning when the Rust compiler is about to introduce newer
  lints/warnings.

- Build on a couple of distros.  Librsvg's test suite is especially
  sensitive to changes in rendering from Cairo, Pixman, and the
  Pango/Freetype2/Harfbuzz stack.  Building on a few distros gives us
  slightly different versions of those dependencies, so that we can
  catch breakage early.

- There is an environment for doing a "full build and test", that runs
  the whole test suite, and tests all the artifacts produced by the
  build.  This environment also has all the tools for generating
  documentation, or producing a test coverage report.  As of
  2022/Sept/12 this is the ``opensuse-container@x86_64.stable`` image.

Quick checks
------------



Full test suite and different environments
------------------------------------------

Lints and formatting
--------------------

Test coverage report
--------------------

Release tests
-------------

Generate documentation
----------------------

- C API docs
- Rust API docs
- Internals docs
