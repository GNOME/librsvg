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

* [Verbosity](#verbosity)
* [Debug or release builds](#debug-or-release-builds)
* [Cross-compilation](#cross-compilation)
* [Building with no network access](#building-with-no-network-access)

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

For the Rust part of librsvg, we have an `--enable-debug` flag that
you can pass to the `configure` script.  When enabled, the Rust
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
passing `--enable-debug` to the `configure` script.  This will omit
the `--release` option from Cargo, so that it will build the Rust
sub-library in debug mode.

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

[autotools]: https://autotools.io/index.html
[blog]: https://people.gnome.org/~federico/blog/librsvg-build-infrastructure.html
[maintainer]: README.md#maintainers
[install]: INSTALL
[rust-target-dir]: https://github.com/rust-lang/rust/tree/master/src/librustc_back/target
[cargo-vendor]: https://crates.io/crates/cargo-vendor
[cargo-source-replacement]: http://doc.crates.io/source-replacement.html
[rust-cross]: https://github.com/japaric/rust-cross
[target-json]: https://github.com/japaric/rust-cross#target-specification-files
