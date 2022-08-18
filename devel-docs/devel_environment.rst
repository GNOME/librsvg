Setting up your development environment
=======================================

This chapter will help you set your development environment for librsvg.

System requirements
-------------------

- A 64-bit installation of Linux.

- 8 GB of RAM, or 16 GB recommended if you will be running the full
  test suite frequently.

- Around 10 GB free of hard disk space.

- You can either use `podman`_ to work in a
  containerized setup (this chapter will show you how), or you can
  install librsvg's dependencies by hand.

- Make sure you have ``git`` installed.

- Your favorite text editor.

Downloading the source code
---------------------------

.. code-block:: sh

   git clone https://gitlab.gnome.org/GNOME/librsvg.git


.. _podman_setup:

Setting up with podman
----------------------

An easy way to set up a development environment is to use `podman`_ to
download and run a container image.  This is similar to having a
``chroot`` with all of librsvg's dependencies already set up.

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
     podman run --rm -ti --cap-add=SYS_PTRACE -v $(pwd):/srv/project -w /srv/project $image_name

   Don't forget to run this once inside the container:
     source ci/env.sh

You can cut&paste those commands (from the script's output, not from
this document!).  The first one should give you a shell prompt inside
the container.  The second one will make Rust available in the shell's
environment.

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

- ``-v $(pwd):/srv/project` - Mount the current directory as
  ``/srv/project`` inside the container.  This lets you build from
  your current source tree without first copying it into the
  container; it will be available in ``/srv/project``.

Finally, don't forget to ``source ci/env.sh`` once you are inside ``podman run``.

You can now skip to :ref:`build`.

.. _manual_setup:

Setting up dependencies manually
--------------------------------

FIXME.

.. _build:

Building and testing
--------------------

Make sure you have gone through the steps in :ref:`podman_setup` or
:ref:`manual_setup`.  Then, do the following.

**Normal development:** You can use ``cargo build`` and ``cargo test``
as for a simple Rust project; this is what you will use most of the
time during regular development.  If you are using the podman
container as per above, you should do this in the ``/srv/project``
directory most of the time.

**Doing a full build:** You can use the following:

.. code-block:: sh

   mkdir -p _build
   cd _build
   ../autogen.sh --enable-gtk-doc --enable-vala
   make
   make check

You should only have to do that if you need to run the full test
suite, for the C API tests and the tests for limiting memory
consumption.

.. _podman: https://podman.io/
