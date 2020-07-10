Please do not compile librsvg in a path with spaces to avoid potential
problems during the build and/or during the usage of the librsvg
library.

Please refer to the following GNOME Live! page for more detailed
instructions on building librsvg and its dependencies with Visual C++:

https://live.gnome.org/GTK%2B/Win32/MSVCCompilationOfGTKStack

This set of NMake Makefiles is intended to be used in a librsvg source tree
unpacked from a tarball.  For building, you will need the following libraries
(headers, .lib's, DLLs, EXEs and scripts), with all of their dependencies:

-libxml2
-GLib
-Cairo (The included cairo-gobject integration library is also needed)
-Pango
-Gdk-Pixbuf
-GObject-Introspection (optional, for building/using introspection files)


You will also need the following tools:
-Visual Studio 2013 or later, with C/C++ compilation support (MSVC).
-The Rust Compiler and tools with the msvc toolchain(s) installed, that
 matches the architecture that is being built.  It is recommended to use the
 'rustup' tool from https://www.rust-lang.org/ to install and configure
 Rust, which will install Rust in %HOMEPATH%\.cargo by default.
-Python (optional, recommended, to generate the pkg-config files and
 build the introspection files; if building the introspection files, the
 Python installation must match the version, architecture and configuration
 of the Python installation that was used to build your copy of
 GObject-Introspection).  Note that introspection files cannot be built in
 for builds that produce binaries that are not compatible with the running
 system where the build is being carried out.  This means, specifically,
 introspection files for ARM64 builds are not currently supported also due to a
 lack of an official native ARM64 Python build.
-For introspection builds, the pkg-config (or compatible) tool is also needed
 and the introspection files and pkg-config files for the dependent libraries
 (if applicable) are also needed.  You will need to set PKG_CONFIG_PATH
 if the pkg-config files cannot be found from the default locations that
 pkg-config will look for.

It is now possible to cross-compile librsvg for ARM64 Windows, as well as for
x64 Windows on 32-bit or ARM64 Windows systems, using this set of NMake Makefiles.
You will need to ensure that the Visual Studio ARM64 and/or x64 cross compiler
appropriate for your system is installed, and you have installed the
`aarch64-pc-windows-msvc` and/or 'x86_64-pc-windows-msvc' target (rust-std library)
via `rustup` for your Rust toolchain.  Similarly, you may choose to use an x86-to-x64
Visual Studio cross compiler, even on an x64 Windows system, so this will also require
that you have installed the 'x86_64-pc-windows-msvc' target for your currently-active
Rust toolchain (see 'rustup default').  Such builds can be carried out on a normal
x86/x86-64 Windows 7+ or on Windows 10 ARM64.

It is recommended that the dependent libraries are built with the same version
of Visual Studio that is being used to build librsvg, as far as possible.

If building from a git checkout is desired, you will need to open the following
*.in files, and replace any items that are surrounded by the '@' characters,
and save those files without the .in file extension:

(open file)                           -> (save as file)
===========                              ==============
config-msvc.mak.in                    -> config-msvc.mak
config.h.win32.in                     -> config.h.win32
$(srcroot)\librsvg\rsvg-features.h.in -> $(srcroot)\librsvg\rsvg-features.h

From this directory in a Visual Studio command prompt, run the following:

 nmake /f Makefile.vc CFG=<CFG> <target> <path_options> <other_options>

Where:
<CFG> is the build configuration, i.e. release or debug.  This is mandatory
for all targets.

<target> is as follows:
-(not specified): builds the librsvg DLL and tools and GDK-Pixbuf SVG loader.
                  If `INTROSPECTION=1` is specified, this will also build
                  the introspection files (.gir/.typelib) for librsvg.
-all: see (not specified).
-tests: Same as (not specified) but also builds the test programs in $(srcroot)\tests.
        You will need the FreeType and HarfBuzz headers, libraries and DLLs (if applicable)
        to build and run this successfully, even if you are building without PangoFT2.
-rsvg_rust_tests: Makes a build of the rust items into an executable to test the rust
                  bits.  You may need to make a copy of libxml2.lib (or so) to xml2.lib
                  in order to build this successfully.
-clean: Removes all build files
-install: Same as (not specified) and also copies the built DLLs, .lib's, headers,
          tools and possibly introspection files to appropriate locations under
          $(PREFIX).  This will also create and copy the librsvg-2.0.pc pkg-config
          file if Python can be found.

<path_options> is as follows:
-PREFIX: Root directory where built files will be copied to with the 'install' target.
         This also determines the root directory from which the dependent headers,
         .lib's and DLLs/.typelib's/.gir's are looked for, if INCLUDEDIR, LIBDIR and/or
         BINDIR are not respectively specified.  Default is
         $(srcroot)\..\vs<vs_short_ver>\<arch>, where vs_short_ver is 12 for Visual
         Studio 2013, 14 for VS 2015, 15 for VS 2017 and 16 for VS 2019.
-INCLUDEDIR: Base directory where headers are looked for, which is PREFIX\include by
             default.  Note that GLib headers are looked for in INCLUDEDIR\glib-2.0
             and LIBDIR\glib-2.0\include.
-LIBDIR: Base directory where .lib's and arch-dependent headers are looked for, which
         is PREFIX\lib by default.
-BINDIR: Base directory where dependent DLLs and tools (.exe's and scripts) are looked
         for, which is PREFIX\bin by default.  Note that for introspection builds,
         this means the introspection Python scripts and modules are in
         BINDIR\..\lib\gobject-introspection and the dependent introspection files are
         looked for in BINDIR\..\share\gir-1.0 and BINDIR\..\lib\girepository-1.0
         respectively for .gir files and .typelib files.
-PYTHON: Path to your Python interpreter executable, if not already in your %PATH% or
         using a different installation of Python is desired.  Please see note above
         on Python usage.  If Python cannot be found, you will not be able to build
         introspection files and the librsvg-2.0.pc pkg-config file will not be
         generated using the 'install' build target.
-PKG_CONFIG: Path to your pkg-config (or compatible) tool, if not already in your
             %PATH%.  This is required for introspection builds.
-LIBINTL_LIB: Full file name of your gettext-runtime library .lib file, if it is not
              intl.lib.  This should be in the directories indicated by %LIB% or in
              $(LIBDIR), or should be passed in with the full path.  Note that its
              DLL, if applicable, should be found in %PATH% or in $(BINDIR) as well,
              for building the introspection files or for creating the GDK-Pixbuf
              loaders cache file.

<other_options> is as follows, activate the options using <option>=1:
-INTROSPECTION: Build the introspection files.  Please see notes above.
-USE_PANGOFT2: Build the test programs with PangoFT2 support, which will enable
               more features to be tested.  This will additionally require Pango
               built with FreeType support, meaning that HarfBuzz, FontConfig and
               FreeType will also be required for the test programs to run.
