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

Describe ci-templates; copy most of the stuff from at-spi2-core's CI README.

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
