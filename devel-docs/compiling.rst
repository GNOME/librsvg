Detailed compilation instructions
=================================

A full build of librsvg requires
`autotools <https://autotools.info/index.html>`__. A full build will
produce these (see :doc:`product` for details):

-  ``rsvg-convert`` binary and its ``man`` page.
-  librsvg shared library with the GObject-based API.
-  Gdk-pixbuf loader for SVG files.
-  HTML documentation for the GObject-based API, with ``gi-docgen``.
-  GObject-introspection information for language bindings.
-  Vala language bindings.

Librsvg uses a mostly normal `autotools
<https://autotools.info/index.html>`__ setup. The historical details of
how librsvg integrates Cargo and Rust into its autotools setup are
described in `this blog post
<https://viruta.org/librsvgs-build-infrastructure-autotools-and-rust.html>`__,
although hopefully you will not need to refer to it.

It is perfectly fine to `ask the maintainer
<https://gitlab.gnome.org/GNOME/librsvg/-/blob/main/README.md#maintainers>`_
if you have questions about the Autotools setup; it’s a tricky bit of
machinery, and we are glad to help.

The rest of this document explains librsvg’s peculiarities apart from
the usual way of compiling Autotools projects:

- `Basic compilation instructions <#basic-compilation-instructions>`_
- `Verbosity <#verbosity>`_
- `Debug or release builds <#debug-or-release-builds>`_
- `Selecting a Rust toolchain <#selecting-a-rust-toolchain>`_
- `Cross-compilation <#cross-compilation>`_
- `Building with no network access <#building-with-no-network-access>`_
- `Running "make distcheck" <#running-make-distcheck>`_

Basic compilation instructions
------------------------------

If you are compiling a tarball:

.. code:: sh

   ./configure --enable-gtk-doc --enable-vala
   make
   make install

See the ``INSTALL`` file for details on options you can
pass to the ``configure`` script to select where to install the compiled
library.

If you are compiling from a git checkout:

.. code:: sh

   ./autogen.sh --enable-gtk-doc --enable-vala
   make
   make install

The ``--enable-gtk-doc`` and ``--enable-vala`` arguments are
optional. They will check that you have gi-docgen and the Vala compiler
installed, respectively.

Verbosity
---------

By default the compilation process is quiet, and it just tells you which
files it is compiling.

If you wish to see the full compilation command lines, use
“``make V=1``” instead of plain “``make``”.

Debug or release builds
-----------------------

Librsvg’s artifacts have code both in C and Rust, and each language has
a different way of specifying compilation options to select compiler
optimizations, or whether debug information should be included.

-  **Rust code:** the librsvg shared library, and the ``rsvg-convert``
   binary. See below.

-  **C code:** the gdk-pixbuf loader; you should set the ``CFLAGS``
   environment variable with compiler flags that you want to pass to the
   C compiler.

Controlling debug or release mode for Rust
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

-  With a ``configure`` option: ``--enable-debug`` or
   ``--disable-debug``
-  With an environment variable: ``LIBRSVG_DEBUG=yes`` or
   ``LIBRSVG_DEBUG=no``

For the Rust part of librsvg, we have a flag that you can pass at
``configure`` time. When enabled, the Rust sub-library will have
debugging information and no compiler optimizations. **This flag is off
by default:** if the flag is not specified, the Rust sub-library will be
built in release mode (no debug information, full compiler
optimizations).

The rationale is that people who already had scripts in place to build
binary packages for librsvg, generally from release tarballs, are
already using conventional machinery to specify C compiler options, such
as that in RPM specfiles or Debian source packages. However, they may
not contemplate Rust libraries and they will certainly not want to
modify their existing packaging scripts too much.

So, by default, the Rust library builds in **release mode**, to make
life easier to binary distributions. Librsvg’s build scripts will add
``--release`` to the Cargo command line by default.

Developers can request a debug build of the Rust code by passing
``--enable-debug`` to the ``configure`` script, or by setting the
``LIBRSVG_DEBUG=yes`` environment variable before calling ``configure``.
This will omit the ``--release`` option from Cargo, so that it will
build the Rust sub-library in debug mode.

In case both the environment variable and the command-line option are
specified, the command-line option overrides the env var.

Selecting a Rust toolchain
--------------------------

By default, the configure/make steps will use the ``cargo`` binary that
is found in your ``$PATH``. If you have a system installation of Rust
and one in your home directory, or for special build systems, you may
need to override the locations of ``cargo`` and/or ``rustc``. In this
case, you can set any of these environment variables before running
``configure`` or ``autogen.sh``:

-  ``RUSTC`` - path to the ``rustc`` compiler
-  ``CARGO`` - path to ``cargo``

Note that ``$RUSTC`` only gets used in the ``configure`` script to
ensure that there is a Rust compiler installed with an appropriate
version. The actual compilation process just uses ``$CARGO``, and
assumes that that ``cargo`` binary will use the same Rust compiler as
the other variable.

Cross-compilation
-----------------

If you need to cross-compile librsvg, specify the ``--host=TRIPLE`` to
the ``configure`` script as usual with Autotools. This will cause
librsvg’s build scripts to automatically pass ``--target=TRIPLE`` to
``cargo``.

Note, however, that Rust may support different targets than the C
compiler on your system. Rust’s supported targets can be found in the
`rustc
manual <https://doc.rust-lang.org/nightly/rustc/platform-support.html>`__

You can check Jorge Aparicio’s `guide on cross-compilation for
Rust <https://github.com/japaric/rust-cross>`__ for more details.

Overriding the Rust target name
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

If you need ``cargo --target=FOO`` to obtain a different value from the
one you specified for ``--host=TRIPLE``, you can use the ``RUST_TARGET``
variable, and this will be passed to ``cargo``. For example,

.. code:: sh

   RUST_TARGET=aarch64-unknown-linux-gnu ./configure --host=aarch64-buildroot-linux-gnu
   # will run "cargo --target=aarch64-unknown-linux-gnu" for the Rust part

Cross-compiling to a target not supported by Rust out of the box
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

When building with a target that is not supported out of the box by
Rust, you have to do this:

1. Create a `target JSON definition
   file <https://github.com/japaric/rust-cross/blob/master/README.md#target-specification-files>`_.

2. Set the environment variable ``RUST_TARGET_PATH`` to its directory
   for the ``make`` command.

Example:

.. code:: sh

   cd /my/target/definition
   echo "JSON goes here" > MYMACHINE-VENDOR-OS.json
   cd /source/tree/for/librsvg
   ./configure --host=MYMACHINE-VENDOR-OS
   make RUST_TARGET_PATH=/my/target/definition

Cross-compiling for win32 target
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

You can also cross-compile to win32 (Microsoft Windows) target by using
`MinGW-w64 <https://www.mingw-w64.org/>`_. You need to specify the
appropriate target in the same way as usual:

-  Set an appropriate target via the ``--host`` configure option:

   -  ``i686-w64-mingw32`` for 32-bit target
   -  ``x86_64-w64-mingw32`` for 64-bit target

-  Set an appropriate RUST_TARGET:

   -  ``i686-pc-windows-gnu`` for 32-bit target
   -  ``x86_64-pc-windows-gnu`` for 64-bit target

For example:

.. code:: sh

   ./configure \
     --host=x86_64-w64-mingw32 \
     RUST_TARGET=x86_64-pc-windows-gnu
   make

The most painful aspect of this way of building is preparing a win32
build for each of librsvg’s dependencies. `MXE <https://mxe.cc/>`__ may
help you on this work.

Building with no network access
-------------------------------

Automated build systems generally avoid network access so that they can
compile from known-good sources, instead of pulling random updates from
the net every time. However, normally Cargo likes to download
dependencies when it first compiles a Rust project.

You can use `cargo vendor
<https://doc.rust-lang.org/cargo/commands/cargo-vendor.html>`_ to
download librsvg's Rust dependencies ahead of time, so that subsequent
compilation don't require network access.

Build systems can use `Cargo’s source replacement
mechanism <https://doc.rust-lang.org/cargo/reference/source-replacement.html>`_ to override
the location of the source code for the Rust dependencies, for example,
in order to patch one of the Rust crates that librsvg uses internally.

Running ``make distcheck``
--------------------------

The ``make distcheck`` command will built a release tarball, extract it,
compile it and test it. However, part of the ``make install`` process
within that command will try to install the gdk-pixbuf loader in your
system location, and it will fail.

Please run ``make distcheck`` like this:

::

   $ make distcheck DESTDIR=/tmp/foo

That ``DESTDIR`` will keep the gdk-pixbuf loader installation from
trying to modify your system locations.
