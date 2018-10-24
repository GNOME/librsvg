Compiling librsvg
=================

Librsvg uses a mostly normal [autotools] setup, but it has some
peculiarities due to librsvg's use of a Rust sub-library.  The details
of how librsvg integrates Cargo and Rust into its autotools setup are
described in [this blog post][blog], although hopefully you will not
need to refer to it.

It is perfectly fine to [ask the maintainer][maintainer] if you have
questions about the Autotools setup; it's a tricky bit of machinery,
and we are glad to help.

There are generic compilation/installation instructions in the
[`INSTALL`][install] file, which comes from Autotools.  The following
explains librsvg's peculiarities.

* [Installing dependencies for building](#installing-dependencies-for-building)
* [Basic compilation instructions](#basic-compilation-instructions)
* [Verbosity](#verbosity)
* [Debug or release builds](#debug-or-release-builds)
* [Cross-compilation](#cross-compilation)
* [Building with no network access](#building-with-no-network-access)
* [Running `make distcheck`](#running-make-distcheck)

# Installing dependencies for building

To compile librsvg, you need the following packages installed.  The
minimum version is listed here; you may use a newer version instead.

**Compilers:**

* a C compiler and `make` tool; we recommend GNU `make`.
* rust 1.27 or later
* cargo

**Mandatory dependencies:**

* Cairo 1.15.12 with PNG support
* Freetype2 2.8.0
* Libcroco 0.6.1
* Gdk-pixbuf 2.20.0
* GIO 2.24.0
* GObject-Introspection 0.10.8
* Libxml2 2.9.0
* Pango 1.38.0

**Optional dependencies:**

* GTK+ 3.10.0 if you want the `rsvg-view-3` program

The following sections describe how to install these dependencies on
several systems.

### Debian based systems

As of 2018/Feb/22, librsvg cannot be built in `debian stable` and
`ubuntu 18.04`, as they have packages that are too old.

**Build dependencies on Debian Testing or Ubuntu 18.10:**

```sh
apt-get install -y gcc make rustc cargo \
automake autoconf libtool gettext itstool \
libgdk-pixbuf2.0-dev libgirepository1.0-dev \
gtk-doc-tools git libgtk-3-dev \
libxml2-dev libcroco3-dev libcairo2-dev libpango1.0-dev
```

### Fedora based systems

```sh
dnf install -y gcc rust rust-std-static cargo make \
automake autoconf libtool gettext itstool \
gdk-pixbuf2-devel gobject-introspection-devel \
gtk-doc git redhat-rpm-config gtk3-devel \
libxml2-devel libcroco-devel cairo-devel pango-devel
```

### openSUSE based systems

```sh
zypper install -y gcc rust rust-std cargo make \
automake autoconf libtool gettext itstool git \
gtk-doc gobject-introspection-devel gtk3-devel \
libxml2-devel libcroco-devel cairo-devel \
pango-devel gdk-pixbuf-devel
```

### macOS systems

Dependencies may be installed using [Homebrew](https://brew.sh) or another
package manager.

```sh
brew install cairo gdk-pixbuf glib libcroco pango \
gobject-introspection rust

export PKG_CONFIG_PATH="`brew --prefix`/lib/pkgconfig:\
`brew --prefix libffi`/lib/pkgconfig:\
/usr/lib/pkgconfig"
export ARCHFLAGS="-arch x86_64"
```

Note that `PKG_CONFIG_PATH` must be manually set to include Homebrew's libffi,
as the system libffi is too old but Homebrew does not install it in a public
location by default.

Currently, cairo 1.15.4 or later must also be installed manually, as the
Homebrew package is for the older stable release. This may require adding
it to `PKG_CONFIG_PATH` as well if you do not install it in `/usr/local`.

Setting `ARCHFLAGS` is required if gobject-introspection is using the system
Python provided by Apple, as on Homebrew.


# Basic compilation instructions

If you are compiling a tarball:

```sh
./configure
make
make install
```

See the [`INSTALL`][install] file for details on options you can pass
to the `configure` script to select where to install the compiled
library.

If you are compiling from a git checkout:

```sh
./autogen.sh
make
make install
```

# Verbosity

By default the compilation process is quiet, and it just tells you
which files it is compiling.

If you wish to see the full compilation command lines, use "`make V=1`"
instead of plain "`make`".

# Debug or release builds

Librsvg has code both in C and Rust, and each language has a different
way of specifying compilation options to select compiler
optimizations, or whether debug information should be included.

You should set the `CFLAGS` environment variable with compiler flags
that you want to pass to the C compiler.

## Controlling debug or release mode for Rust

* With a `configure` option: `--enable-debug` or `--disable-debug`
* With an environment variable: `LIBRSVG_DEBUG=yes` or `LIBRSVG_DEBUG=no`

For the Rust part of librsvg, we have a flag that
you can pass at `configure` time.  When enabled, the Rust
sub-library will have debugging information and no compiler
optimizations.  *This flag is off by default:* if the flag is not
specified, the Rust sub-library will be built in release mode (no
debug information, full compiler optimizations).

The rationale is that people who already had scripts in place to build
binary packages for librsvg, generally from release tarballs, are
already using conventional machinery to specify C compiler options,
such as that in RPM specfiles or Debian source packages.  However,
they may not contemplate Rust sub-libraries and they will certainly
not want to modify their existing packaging scripts too much.

So, by default, the Rust library builds in **release mode**, to make
life easier to binary distributions.  Librsvg's build scripts will add
`--release` to the Cargo command line by default.

Developers can request a debug build of the Rust sub-library by
passing `--enable-debug` to the `configure` script, or by setting the
`LIBRSVG_DEBUG=yes` environment variable before calling `configure`.
This will omit the `--release` option from Cargo, so that it will
build the Rust sub-library in debug mode.

In case both the environment variable and the command-line option are
specified, the command-line option overrides the env var.

# Cross-compilation

If you need to cross-compile librsvg, specify the `--host=TRIPLE` to
the `configure` script as usual with Autotools.  This will cause
librsvg's build scripts to automatically pass `--target=TRIPLE` to
`cargo`.

Note, however, that Rust may support different targets than the C
compiler on your system.  Rust's supported targets can be found in the
[`rust/src/librustc_back/target`][rust-target-dir] in the Rust
compiler's source code.

You can check Jorge Aparicio's [guide on cross-compilation for
Rust][rust-cross] for more details.

## Overriding the Rust target name

If you need `cargo --target=FOO` to obtain a different value from the
one you specified for `--host=TRIPLE`, you can use the `RUST_TARGET`
variable, and this will be passed to `cargo`.  For example,

```sh
RUST_TARGET=aarch64-unknown-linux-gnu ./configure --host=aarch64-buildroot-linux-gnu
# will run "cargo --target=aarch64-unknown-linux-gnu" for the Rust part
```

## Cross-compiling to a target not supported by Rust out of the box

When building with a target that is not supported out of the box by
Rust, you have to do this:

1. Create a [target JSON definition file][target-json].

2. Set the environment variable `RUST_TARGET_PATH` to its directory
   for the `make` command.

Example:

```sh
cd /my/target/definition
echo "JSON goes here" > MYMACHINE-VENDOR-OS.json
cd /source/tree/for/librsvg
./configure --host=MYMACHINE-VENDOR-OS
make RUST_TARGET_PATH=/my/target/definition
```

# Building with no network access

Automated build systems generally avoid network access so that they
can compile from known-good sources, instead of pulling random updates
from the net every time.  However, normally Cargo likes to download
dependencies when it first compiles a Rust project.

We use [`cargo vendor`][cargo-vendor] to ship librsvg release tarballs
with the source code for Rust dependencies **embedded within the
tarball**.  If you unpack a librsvg tarball, these sources will appear
in the `rust/vendor` subdirectory.  If you build librsvg from a
tarball, instead of git, it should not need to access the network to
download extra sources at all.

Build systems can use [Cargo's source replacement
mechanism][cargo-source-replacement] to override the location of the
source code for the Rust dependencies, for example, in order to patch
one of the Rust crates that librsvg uses internally.

The source replacement information is in `rust/.cargo/config` in the
unpacked tarball.  Your build system can patch this file as needed.

# Running `make distcheck`

The `make distcheck` command will built a release tarball, extract it,
compile it and test it.  However, part of the `make install` process
within that command will try to install the gdk-pixbuf loader in your
system location, and it will fail.

Please run `make distcheck` like this:

```
$ make distcheck DESTDIR=/tmp/foo
```

That `DESTDIR` will keep the gdk-pixbuf loader installation from
trying to modify your system locations.

[autotools]: https://autotools.io/index.html
[blog]: https://people.gnome.org/~federico/blog/librsvg-build-infrastructure.html
[maintainer]: README.md#maintainers
[install]: INSTALL
[rust-target-dir]: https://github.com/rust-lang/rust/tree/master/src/librustc_back/target
[cargo-vendor]: https://crates.io/crates/cargo-vendor
[cargo-source-replacement]: http://doc.crates.io/source-replacement.html
[rust-cross]: https://github.com/japaric/rust-cross
[target-json]: https://github.com/japaric/rust-cross#target-specification-files
