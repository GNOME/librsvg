How to contribute
=================

Thank you for looking in this document! There are different ways of
contributing to librsvg, and we appreciate all of them.

All librsvg contributors are expected to follow `GNOME's Code of
Conduct <https://conduct.gnome.org>`_.

Source repository
-----------------

Librsvg’s main source repository is at ``gitlab.gnome.org``. You can view
the web interface here:

https://gitlab.gnome.org/GNOME/librsvg

Development happens in the ``main`` branch. There are also branches for
stable releases.

Alternatively, you can use the mirror at GitHub:

https://github.com/GNOME/librsvg

Note that we don’t do bug tracking in the GitHub mirror; see the next
section.

If you need to publish a branch, feel free to do it at any
publicly-accessible Git hosting service, although ``gitlab.gnome.org``
makes things easier for the maintainers of librsvg.

Hacking on librsvg
------------------

See the rest of this development guide, especially the chapter on
:doc:`architecture`, and the tutorial on :doc:`adding_a_property`.

The library’s internals are being documented at
https://gnome.pages.gitlab.gnome.org/librsvg/internals/rsvg/index.html

What can you hack on?

- `Bugs for
  newcomers <https://gitlab.gnome.org/GNOME/librsvg/-/issues?label_name%5B%5D=4.+Newcomers>`__.
- `Bugs for intermediate developers, once you are a bit familiar with
  the code
  <https://gitlab.gnome.org/GNOME/librsvg/-/issues/?label_name%5B%5D=Intermediate>`__.
- Pick something from the `development
  roadmap <https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/roadmap.html>`__.
- Tackle one of the `bigger projects
  <https://gitlab.gnome.org/GNOME/librsvg/-/issues/?label_name%5B%5D=project>`_.

Working on the source
~~~~~~~~~~~~~~~~~~~~~

Make sure you have read the chapter on :doc:`devel_environment`.

A typical development cycle goes like this:

- Make some changes to the code; hopefully adding a test first.
- Build and run the tests.  Use ``cargo test --workspace``.
- Repeat until you and the tests are happy.

Most of the time you can just work on the Rust source code and use
``cargo test --workspace``.  This will run the Rust-based tests, which
are usually enough to test bug fixes in the SVG rendering code.

However, sometimes you'll want to do a full build and run the full
test suite, which includes the tests for the C API.

To do a full build, you can use something like this:

.. code-block:: sh

   mkdir -p _build
   meson setup _build -Ddocs=enabled -Dintrospection=enabled -Dvala=enabled
   meson compile -C _build
   meson test -C _build

Alternatively, you can make a merge request and wait for the
Continuous Integration machinery (CI) do the full build for you.  The
CI will run all the tests on multiple platforms.

For a full build, librsvg uses the `Meson <https://mesonbuild.com>`_
build system.  If you need to **add a new source file**, you need to
do it in ``rsvg/meson.build``.  This is so that Meson can know when a
Rust file changed so it can call ``cargo`` as appropriate.

It is perfectly fine to ask the maintainer if you have questions about
the Meson setup; it’s a tricky bit of machinery, and we are glad
to help.

Continuous Integration
~~~~~~~~~~~~~~~~~~~~~~

If you fork librsvg in ``gitlab.gnome.org`` and push commits to your
forked version, the Continuous Integration machinery (CI) will run
automatically.

The CI infrastructure is documented in the :doc:`ci` chapter.

When you create a merge request, or push to a branch in a fork of
librsvg, GitLab's CI will run a *pipeline* on the contents of your
push: it will run the test suite, linters, try to build the
documentation, and generally see if everything that makes
:doc:`product` is working as intended.  If any tests fail, the
pipeline will fail and you can then examine the build artifacts of
failed jobs to fix things.

**Automating the code formatting:** You may want to enable a
`client-side git
hook <https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks>`__ to
run ``rustfmt`` before you can commit something; otherwise the ``lint``
stage of CI pipelines will fail:

1. ``cd librsvg``

2. ``mv .git/hooks/pre-commit.sample .git/hooks/pre-commit``

3. Edit ``.git/hooks/pre-commit`` and put in one of the following
   commands:

   -  If you want code reformatted automatically, no questions asked:
      ``cargo fmt``.
     
      .. note::

         If this actually reformats your code while committing, you’ll
         have to re-stage the new changes and ``git commit --amend``.
         Be careful if you had unstaged changes that got reformatted!

   -  If you want to examine errors if rustfmt doesn’t like your
      indentation, but don’t want it to make changes on its own:
      ``cargo fmt --all -- --check``

Test suite
~~~~~~~~~~

All new features need to have corresponding tests.  Please see the
file ``rsvg/tests/README.md`` to see how to add new tests to the test suite.  In short:

- Add unit tests in the ``rsvg/src/*.rs`` files for internal things like
  parsers or algorithms.

- Add rendering tests in ``rsvg/tests/src/*.rs`` for SVG or CSS features.
  See ``rsvg/tests/README.md`` for details on how to do this.

- Tests for the C API go in ``librsvg-c/test-c/*.c``.  Note that to
  run these tests you must run a full meson build, not just ``cargo
  test --workspace``.

- Tests for ``rsvg-convert`` go in ``rsvg_convert/tests/*.rs``.

In most cases, you can run ``cargo test --workspace`` if you set up your
development environment as instructed in the :doc:`devel_environment`
chapter.  Alternatively, push your changes to a branch, and watch the
results of its CI pipeline.

Creating a merge request
~~~~~~~~~~~~~~~~~~~~~~~~

You may create a forked version of librsvg in `GNOME’s Gitlab instance
<https://gitlab.gnome.org/GNOME/librsvg>`__,. You can register an
account there, or log in with your account from other OAuth services.

For technical reasons, the maintainers of librsvg do not get
automatically notified if you submit a pull request through the GNOME
mirror in GitHub.  In that case, please create a merge request at
``gitlab.gnome.org`` instead; you can ask the maintainer for assistance.

Formatting commit messages
~~~~~~~~~~~~~~~~~~~~~~~~~~

If a commit fixes a bug, please format its commit message like this:

::

   (#123): Don't crash when foo is bar

   Explanation for why the crash happened, or anything that is not
   obvious from looking at the diff.

   Fixes https://gitlab.gnome.org/GNOME/librsvg/issues/123

Note the ``(#123)`` in the first line. This is the line that shows up in
single-line git logs, and having the bug number there makes it easier to
write the release notes later — one does not have to read all the commit
messages to find the ids of fixed bugs.

Also, please paste the complete URL to the bug report somewhere in the
commit message, so that it’s easier to visit when reading the commit
logs.

Generally, commit messages should summarize *what* you did, and *why*.
Think of someone doing ``git blame`` in the future when trying to figure
out how some code works: they will want to see *why* a certain line of
source code is there. The commit where that line was introduced should
explain it.

Testing performance-related changes
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The ``rsvg-bench`` directory in the source tree has a tool to
benchmark librsvg.  For example, you can ask rsvg-bench to render one
or more SVGs hundreds of times in a row, so you can take accurate
timings or run a sampling profiler and get enough samples.

To fully read on and understand the usage of 
``rsvg-bench``, please refer to the :doc:`rsvg_bench` chapter.

It has a comprehensive detail on how to use the tool, and how to
interpret the results.

Included benchmarks
~~~~~~~~~~~~~~~~~~~

The ``rsvg/benches/`` directory has a couple of benchmarks for functions
related to SVG filter effects.  You can run them with ``cargo bench``.

These benchmarks use the
`Criterion <https://crates.io/crates/criterion>`__ crate, which supports
some interesting options to generate plots and such.
