RSVG-BENCH COMMAND-LINE OPTIONS
===============================

The `rsvg-bench <https://gitlab.gnome.org/GNOME/librsvg/-/tree/main/rsvg-bench>`_ program is a simple benchmarking tool for librsvg.

It is a small utility for benchmarking `librsvg <https://gitlab.gnome.org/GNOME/librsvg>`_ which 
is `Gnome <https://gitlab.gnome.org/GNOME>`_'s library for rendering
Scalable Vector Graphics (`SVG <https://en.wikipedia.org/wiki/Scalable_Vector_Graphics>`_)

GOALS
=====

To benchmark librsvg, we would like to do several things:

- Be able to process many SVG images with a single command.  For
  example, this lets us answer a question like, "how long does version
  N of librsvg take to render a directory full of SVG icons?" — which
  is important for the performance of an application chooser.

- Be able to **repeatedly** process SVG files, for example, "render this
  SVG 1000 times in a row".  This is useful to get accurate timings,
  as a single render may only take a few microseconds and may be hard
  to measure.  It also helps with running profilers, as they will be
  able to get more useful samples if the SVG rendering process runs
  repeatedly for a long time.

- Exercise librsvg's major code paths for parsing and rendering
  separately.  For example, librsvg uses different parts of the XML
  parser depending on whether it is being pushed data, vs. being asked
  to pull data from a stream.  Also, we may only want to benchmark the
  parser but not the renderer; or we may want to parse SVGs only once
  but render them many times after that.

COMPILING
=========
This benchmark is compiled along with `librsvg`. 
To compile the benchmark, you need to setup you development or test environment 
for the `librsvg` library. You can follow the instructions in 
the `setup development environment page <https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/devel_environment.html>`_.

you can also simply run `cargo build --workspace` in the root of the `librsvg` source tree. 
This command will compile but the `librsvg` library and the `rsvg-bench` benchmark.

USAGE / BENCHMARKING
====================
After compiling `librsvg`, the `rsvg-bench` binary will be available in the `target/debug` 
directory of the `librsvg` source tree. 
You can run the `rsvg-bench` binary with the following command line options:

Running `target/debug/rsvg-bench --help` will display the help message.

.. code-block:: bash

    Benchmarking utility for librsvg.

    Usage: rsvg-bench [OPTIONS] [inputs]...

    Arguments:
        [inputs]...  Input files or directories

    Options:
        --sleep <sleep>            Number of seconds to sleep before starting to process SVGs [default: 0]
        --num-parse <num-parse>    Number of times to parse each file [default: 1]
        --num-render <num-render>  Number of times to render each file [default: 1]
        --hard-failures            Stop all processing when a file cannot be rendered
    -h, --help                     Print help
    -V, --version                  Print version



BENCHMARKING FILES
==================
.. code-block:: bash

    target/debug/rsvg-bench /path/to/svg/files

This command will benchmark the rendering of all the SVG files in the directory `/path/to/svg/files`. 
The benchmark will parse each file once and render it once.
    
.. code-block:: bash

    hard_failures: false
    Will parse each file 1 times
    Will render each file 1 times
    Rendering to Cairo image surface
    Sleeping for 0 seconds before processing SVGs...
    Processing files!
    Processing "rsvg/tests/fixtures/text/"
    Processing "rsvg/tests/fixtures/text/hello-world.svg"
    Processing "rsvg/tests/fixtures/text/bounds-ref.svg"
    Processing "rsvg/tests/fixtures/text/display-none.svg"
    Processing "rsvg/tests/fixtures/text/visibility-hidden.svg"
    Processing "rsvg/tests/fixtures/text/visibility-hidden-ref.svg"
    Processing "rsvg/tests/fixtures/text/span-bounds-when-offset-by-dx.svg"
    Processing "rsvg/tests/fixtures/text/bug806-text-anchor-chunk.svg"
    Processing "rsvg/tests/fixtures/text/span-bounds-when-offset-by-dx-ref.svg"
    Processing "rsvg/tests/fixtures/text/visibility-hidden-x-attr.svg"
    Processing "rsvg/tests/fixtures/text/unicode-bidi-override.svg"
    Processing "rsvg/tests/fixtures/text/display-none-ref.svg"
    Processing "rsvg/tests/fixtures/text/bug804-tspan-direction-change-ref.svg"
    Processing "rsvg/tests/fixtures/text/unicode-bidi-override-ref.svg"
    Processing "rsvg/tests/fixtures/text/bug804-tspan-direction-change.svg"
    Processing "rsvg/tests/fixtures/text/bug806-text-anchor-chunk-ref.svg"
    Processing "rsvg/tests/fixtures/text/bounds.svg"
    0.28user 0.05system 0:00.29elapsed 114%CPU (0avgtext+0avgdata 31912maxresident)k
    136inputs+0outputs (2major+1941minor)pagefaults 0swaps

The output will show the time taken to render each file. The time is in seconds, 
the number of times each files are parsed and rendered, and the number of files that were processed.

.. code-block:: bash
    target/debug/rsvg-bench /path/to/svg/files/file.svg

This command will benchmark the rendering of a single SVG file `/path/to/svg/files/file.svg`. 
The benchmark will parse the file once and render it once.

while you can also specify multiple files to benchmark by providing the path to each file as an argument to the `target/debug/rsvg-bench` command.

.. code-block:: bash
    target/debug/rsvg-bench /path/to/svg/files/file1.svg /path/to/svg/files/file2.svg /path/to/svg/files/file3.svg

This command will benchmark the rendering of the SVG files `/path/to/svg/files/file1.svg`, `/path/to/svg/files/file2.svg`, 
and `/path/to/svg/files/file3.svg`. The benchmark will parse each file once and render it once.

.. code-block:: bash

    hard_failures: false
    Will parse each file 1 times
    Will render each file 1 times
    Rendering to Cairo image surface
    Sleeping for 0 seconds before processing SVGs...
    Processing files!
    Processing "/path/to/svg/files/file1.svg"
    Processing "/path/to/svg/files/file2.svg"
    Processing "/path/to/svg/files/file3.svg"
    0.28user 0.05system 0:00.29elapsed 114%CPU (0avgtext+0avgdata 31912maxresident)k
    136inputs+0outputs (2major+1941minor)pagefaults 0swaps


BENCHMARKING WITH OPTIONS
=========================

The `rsvg-bench` binary has several command line options that can be used to customize the benchmarking process. 
They are listed above when we ran the `--help` option with the `target/debug/rsvg-bench` command.
These options are:

- `--sleep <sleep>`: Number of seconds to sleep before starting to process SVGs [default: 0]
- `--num-parse <num-parse>`: Number of times to parse each file [default: 1]
- `--num-render <num-render>`: Number of times to render each file [default: 1]
- `--hard-failures`: Stop all processing when a file cannot be rendered

You can ask the `rsvg-bench` to sleep for a number of seconds before processing the SVG files. 
This is useful when you want to give the system some time to settle before 
starting the benchmarking process and also so that you can attach a profiler
to it.  For example, `sysprof <https://blogs.gnome.org/chergert/2016/04/19/how-to-sysprof/>_` 
lets you choose an already-running process to monitor.

.. code-block:: bash
    target/debug/rsvg-bench --sleep 5 /path/to/svg/files

This command will benchmark the rendering of all the SVG files in the directory `/path/to/svg/files`.
The benchmark will parse each file once and render it once. 
The benchmark will sleep for 5 seconds before processing the SVG files.

.. code-block:: bash

    hard_failures: false
    Will parse each file 1 times
    Will render each file 1 times
    Rendering to Cairo image surface
    Sleeping for 5 seconds before processing SVGs...
    Processing files!
    Processing "rsvg/tests/fixtures/text/"
    Processing "rsvg/tests/fixtures/text/hello-world.svg"
    Processing "rsvg/tests/fixtures/text/bounds-ref.svg"
    Processing "rsvg/tests/fixtures/text/display-none.svg"
    Processing "rsvg/tests/fixtures/text/visibility-hidden.svg"
    Processing "rsvg/tests/fixtures/text/visibility-hidden-ref.svg"
    Processing "rsvg/tests/fixtures/text/span-bounds-when-offset-by-dx.svg"
    Processing "rsvg/tests/fixtures/text/bug806-text-anchor-chunk.svg"
    Processing "rsvg/tests/fixtures/text/span-bounds-when-offset-by-dx-ref.svg"
    Processing "rsvg/tests/fixtures/text/visibility-hidden-x-attr.svg"
    Processing "rsvg/tests/fixtures/text/unicode-bidi-override.svg"
    Processing "rsvg/tests/fixtures/text/display-none-ref.svg"
    Processing "rsvg/tests/fixtures/text/bug804-tspan-direction-change-ref.svg"
    Processing "rsvg/tests/fixtures/text/unicode-bidi-override-ref.svg"
    Processing "rsvg/tests/fixtures/text/bug804-tspan-direction-change.svg"
    Processing "rsvg/tests/fixtures/text/bug806-text-anchor-chunk-ref.svg"
    Processing "rsvg/tests/fixtures/text/bounds.svg"
    0.28user 0.05system 0:00.29elapsed 114%CPU (0avgtext+0avgdata 31912maxresident)k
    136inputs+0outputs (2major+1941minor)pagefaults 0swaps


.. code-block:: bash

    target/debug/rsvg-bench --num-parse 2 --num-render 2 /path/to/svg/files

This command will benchmark the rendering of all the SVG files in the directory `/path/to/svg/files`. 
The benchmark will parse each file twice and render it twice.

CODE OF CONDUCT
===============

There is a code of conduct for contributors to rsvg-bench; 
please see the file code-of-conduct.md.
Please report bugs in gitlab.gnome.org's issue tracker. 
You can also fork the repository and create merge requests there.
`code-of-conduct <https://gitlab.gnome.org/federico/rsvg-bench/-/blob/master/code-of-conduct.md>`_

Please report bugs in `gitlab.gnome.org's issue tracker <https://gitlab.gnome.org/GNOME/librsvg/-/issues>`_. 
You can also fork the repository and create merge requests there.

MAINTAINERS
===========

The maintainer of librsvg is `Federico Mena Quintero <https://gitlab.gnome.org/federico>`_.  Feel
free to contact me for any questions you may have about librsvg, both
its usage and its development.  You can contact me in the following
ways: 

- Email him at federico@gnome.org.

- Matrix: He is `@federico` on the `GNOME Hackers <https://matrix.to/#/#gnome-hackers:gnome.org>`_ and
`Rust ❤️ GNOME <https://matrix.to/#/#rust:gnome.org>`_ channels on gnome.org's Matrix.  He is
there most weekdays (Mon-Fri) starting at about UTC 14:00 (that's
08:00 my time; I am in the UTC-6 timezone).  If this is not a
convenient time for you, feel free to mail him and he can
arrange a time.

- He frequently `blog about librsvg <https://viruta.org/tag/librsvg.html>`_.  You may be interested in
the articles about porting librsvg from C to Rust, which happened
between 2016 and 2020.

Documentation is written by `Adetoye Anointing <https://gitlab.gnome.org/Yorubad-Dev>`_ and
`Yorubad-Dev <https://x.com/yorubad-dev>`_ on X (formerly known as twitter). you can also email me at yorubaddev@duck.com