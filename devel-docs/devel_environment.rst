Setting up your development environment
=======================================

This chapter will help you set your development environment for librsvg.

System requirements
-------------------

- A 64-bit installation of Linux.

- 8 GB of RAM, or 16 GB recommended if you will be running the full
  test suite frequently.

- Around 10 GB free of hard disk space.

- You can either use `podman <https://podman.io/>`_ to work in a
  containerized setup (this chapter will show you how), or you can
  install librsvg's dependencies by hand.

- Make sure you have ``git`` installed.

- Your favorite text editor.  We recommend a text editor configured to
  use the Language Server Protocol plus `rust-analyzer
  <https://rust-analyzer.github.io/>`_ so you can get autocompletion
  and documentation of librsvg's internals within your editor.

Downloading the source code
---------------------------

.. code-block:: sh

   git clone https://gitlab.gnome.org/GNOME/librsvg.git


.. _podman_setup:

Setting up with podman
----------------------

An easy way to set up a development environment is to use `podman
<https://podman.io/>`_ to download and run a container image.  This is
similar to having a ``chroot`` with all of librsvg's dependencies
already set up.

Install ``podman`` on your distro, and then:

.. code-block:: sh

   cd librsvg      # wherever you did your "git clone"
   sh ci/pull-container-image.sh

In the librsvg source tree, ``ci/pull-container-image.sh`` is a script
that will invoke ``podman pull`` to download the container image that
you can use for development.  It is the same image that librsvg uses
for its continuous integration pipeline (CI), so you can have exactly
the same setup on your own machine.

That ``pull-container-image.sh`` script will give you instructions
similar to these:

.. code-block:: text

   You can now run this:
     podman run --rm -ti --cap-add=SYS_PTRACE -v $(pwd):/srv/project:z -w /srv/project $image_name

   Don't forget to run this once inside the container:
     source ci/env.sh
     source ci/setup-dependencies-env.sh

You can cut&paste those commands (from the script's output, not from
this document!).  The first one should give you a shell prompt inside
the container.  The second and third ones will make Rust available in
the shell's environment, and adjust some environment variables so that
the compilation process can find the installed dependencies.

What's all that magic?  Let's dissect the podman command line:

- ``podman run`` - run a specific container image.  The image name is
  the last parameter in that command; it will look something like
  ``registry.gitlab.gnome.org/gnome/librsvg/opensuse/tumbleweed:x86_64-1.60.0-2022-08-17.0-main``.
  This is an image built on on a base of the openSUSE Tumbleweed, a
  rolling distribution of Linux with very recent dependencies.

- ``--rm`` - Remove the container after exiting.  It will terminate
  when you ``exit`` the container's shell.

- ``-ti`` - Set up an interactive session.

- ``--cap-add=SYS_PTRACE`` - Make it possible to run ``gdb`` inside the container.

- ``-v $(pwd):/srv/project:z` - Mount the current directory as
  ``/srv/project`` inside the container.  This lets you build from
  your current source tree without first copying it into the
  container; it will be available in ``/srv/project``.  The ``:z`` at
  the end is so that if your host distro uses selinux, it will label
  the mounted volume properly (it does nothing if you are not on
  selinux).

Finally, don't forget to ``source ci/env.sh`` and ``source
ci/setup-dependencies-env.sh`` once you are inside ``podman run``.

You can now skip to :ref:`build`.

.. _manual_setup:

Setting up dependencies manually
--------------------------------

.. include:: _build_dependencies.rst

The following sections describe how to install these dependencies on
several systems.  For fully manual builds, you can try using the
script in ``ci/build-dependencies.sh``.  Librsvg's continuous
integration (CI) infrastructure uses that script to install the
dependencies before building.

Debian based systems
~~~~~~~~~~~~~~~~~~~~

As of 2018/Feb/22, librsvg cannot be built in ``debian stable`` and
``ubuntu 18.04``, as they have packages that are too old.

**Build dependencies on Debian Testing or Ubuntu 18.10:**

.. code-block:: sh

   apt-get install -y gcc rustc cargo cargo-c ninja-build \
   meson gi-docgen python3-docutils git \
   libgdk-pixbuf2.0-dev libgirepository1.0-dev \
   libxml2-dev libcairo2-dev libpango1.0-dev

Additionally, as of September 2018 you need to add ``gdk-pixbuf``
utilities to your path, see :issue:`331` for details:

.. code-block:: sh

   PATH="$PATH:/usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0"

Fedora based systems
~~~~~~~~~~~~~~~~~~~~

.. code-block:: sh

   dnf install -y gcc rust rust-std-static cargo cargo-c ninja-build \
   meson gi-docgen python3-docutils git redhat-rpm-config \
   gdk-pixbuf2-devel gobject-introspection-devel \
   libxml2-devel cairo-devel cairo-gobject-devel pango-devel

openSUSE based systems
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: sh

   zypper install -y gcc rust rust-std cargo cargo-c ninja \
   meson python3-gi-docgen python38-docutils git \
   gdk-pixbuf-devel gobject-introspection-devel \
   libxml2-devel cairo-devel pango-devel

macOS systems
~~~~~~~~~~~~~

Dependencies may be installed using `Homebrew <https://brew.sh>`_ or another
package manager.

.. code-block:: sh

   brew install meson gi-docgen pkgconfig gobject-introspection gdk-pixbuf pango

.. _build:

Building and testing
--------------------

Make sure you have gone through the steps in :ref:`podman_setup` or
:ref:`manual_setup`.  Then, do the following.

**Normal development:** You can use ``cargo build`` and
``cargo test`` as for a simple Rust project; this is what
you will use most of the time during regular development.  If you are
using the podman container as per above, you should do this in the
``/srv/project`` directory most of the time.

After compiling with those commands, you can use the ``rsvg-convert``
binary to casually test rendering an SVG file, for example, one that
has a feature that you are developing.  You can run
``target/debug/rsvg-convert -o output.png my_test_file.svg``.

If you do a release build with ``cargo build --release``, which includes
optimizations, the binary will be in ``target/release/rsvg-convert``.
This version is *much* faster than the debug version.

**Doing a full build:** You can use the following:

.. code-block:: sh

   mkdir -p _build
   meson setup _build -Ddocs=enabled -Dintrospection=enabled -Dvala=enabled
   meson compile -C _build
   meson test -C _build

You should only have to do that if you need to run the full test
suite, for the C API tests and the tests for limiting memory
consumption.



.. _podman: https://podman.io/
