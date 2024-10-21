OSS-Fuzz
========

Platform overview
-----------------

`OSS-Fuzz <https://google.github.io/oss-fuzz/>`_ is Google's free fuzzing platform for open source
software.
It runs librsvg's :source:`fuzz targets <fuzz>` to help
detect reliability issues.

Google provides public `build logs <https://oss-fuzz-build-logs.storage.googleapis.com/index.html#librsvg>`_
and `fuzzing stats <https://introspector.oss-fuzz.com/project-profile?project=librsvg>`_, but most
of the details about bug reports and fuzzed testcases require approved access.

Gaining access
^^^^^^^^^^^^^^

The configuration files for the OSS-Fuzz integration can be found in the
`OSS-Fuzz repository <https://github.com/google/oss-fuzz/tree/master/projects/librsvg>`_.
The ``project.yaml`` file controls who has access to bug reports and testcases.
Ping the maintainer if you'd like to be added to the list (note: a Google account is required for
access).

Fuzzing progress
----------------

Once you have access to OSS-Fuzz, you can log in to https://oss-fuzz.com/ with your Google account
to see a dashboard of librsvg's fuzzing progress.

Testcases
^^^^^^^^^

The dashboard contains a link to a `testcases page <https://oss-fuzz.com/testcases?project=librsvg&open=yes>`_
that lists all testcases that currently trigger a bug in librsvg.

Every testcase has a dedicated page with links to view and download a minimized testcase for
reproducing the failure.
Each testcase page also contains a stacktrace for the failure and stats about how often the failure
is encountered while fuzzing.

Reproducing a failure
"""""""""""""""""""""

You can download a minimized testcase and run it with a local fuzz target to debug a failure on your
machine.
For example, to reproduce a failure with the ``render_document`` fuzz target, you can run a command
like this: ``cargo fuzz run render_document minimized.svg``

Individual fuzz targets can also be run inside of a debugger for further debugging information:

.. code:: bash

  FUZZ_TARGET=$(find ./target/*/release/ -type f -name render_document)
  gdb --args "$FUZZ_TARGET" minimized.svg

If the failure does not reproduce locally, you can try reproducing the issue in an OSS-Fuzz
container:

.. code:: bash

  git clone https://github.com/google/oss-fuzz.git
  cd oss-fuzz

  python infra/helper.py build_image librsvg
  python infra/helper.py build_fuzzers librsvg
  python infra/helper.py reproduce librsvg render_document minimized.svg

Code coverage
^^^^^^^^^^^^^

The dashboard also links to code coverage data for individual fuzz targets and combined code
coverage data for all targets (click on the "TOTAL COVERAGE" link for the combined data).

The combined coverage data is helpful for identifying coverage gaps, insufficient corpus data, and
potential candidates for future fuzz targets.

Bug reports
^^^^^^^^^^^

Bug reports for new failures are automatically filed in the OSS-Fuzz bug tracker with a
`librsvg label <https://issues.oss-fuzz.com/issues?q=project:librsvg%20status:open>`_.
Make sure you are logged in to view all existing issues.

Build maintenance
-----------------

Google runs compiled fuzz targets on Google Compute Engine VMs.
This architecture requires each project to provide a ``Dockerfile`` and ``build.sh`` script to
download code, configure dependencies, compile fuzz targets, and package any corpus files.

librsvg's build files can be found in the
`OSS-Fuzz repo <https://github.com/google/oss-fuzz/blob/master/projects/librsvg/>`_.

If dependencies change or if new fuzz targets are added, then you may need to modify the build files
and build a new Docker image for OSS-Fuzz.

Building an image
^^^^^^^^^^^^^^^^^

Use the following commands to build librsvg's OSS-Fuzz image and fuzz targets:

.. code:: bash

  git clone https://github.com/google/oss-fuzz.git
  cd oss-fuzz

  python infra/helper.py build_image librsvg
  python infra/helper.py build_fuzzers librsvg

Any changes you make to the build files must be submitted as pull requests to the OSS-Fuzz repo.

Debugging build failures
""""""""""""""""""""""""

You can debug build failures during the ``build_fuzzers`` stage by creating a container and manually
running the ``compile`` command:

.. code:: bash

  # Create a container for building fuzz targets
  python infra/helper.py shell librsvg

  # Run this command inside the container to build the fuzz targets
  compile

This approach is faster than re-running the ``build_fuzzers`` command, which recompiles everything
from scratch each time the command is run.

The ``build.sh`` script will be located at ``/src/build.sh`` inside the container.

Quick links
-----------

* `OSS-Fuzz dashboard <https://oss-fuzz.com/>`_
* `OSS-Fuzz configuration files and build scripts for librsvg <https://github.com/google/oss-fuzz/tree/master/projects/librsvg>`_
* `All open OSS-Fuzz bugs for librsvg <https://issues.oss-fuzz.com/issues?q=project:librsvg%20status:open>`_
* `Google's OSS-Fuzz documentation <https://google.github.io/oss-fuzz/>`_
