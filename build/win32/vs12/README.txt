Note that all this is rather experimental.

Please do not compile librsvg in a path with spaces to avoid potential
problems during the build and/or during the usage of the librsvg
library.

Please refer to the following GNOME Live! page for more detailed
instructions on building librsvg and its dependencies with Visual C++:

https://live.gnome.org/GTK%2B/Win32/MSVCCompilationOfGTKStack

This VS12 solution and the projects it includes are intented to be used
in a librsvg source tree unpacked from a tarball. In a git checkout you
first need to use some Unix-like environment or manual work to expand
files as needed, for instance the .vcprojin files here into .vcproj
files.

It is recommended that GLib, libxml2, libcroco, Cairo, Pango is compiled
with VS12 to compile librsvg.

External dependencies are at least Cairo, GLib, libxml2, libcroco, GDK-Pixbuf
Please see the build\win32\vs12\README.txt file in glib for details where to
unpack them.

You will also need the Rust compiler package (https://www.rust-lang.org/), where
binary MSI packages are available.  Note that there are two packages for Windows
for each of the 32-bit and 64-bit variants--you *will* need to use the MSVC ABI
(*not* GNU ABI) package that corresponds to your build configuration (Win32/32-bit
and x64/64-bit).  Do note that the .msi installers will put the path where the
rustc.exe/cargo.exe are installed in your PATH, so it is recommended that you
remove the path(s) from your system's PATH settings and setup your IDE to look
for your 32-bit rustc.exe/cargo.exe for Win32/32-bit configs, and to look for
your 64-bit rustc.exe/cargo.exe for x64/64-bit configs, or append
" CARGO=<full_path_to_cargo.exe>" after the the items in the NMake command lines
in the rsvg-rust project settings.

It is recommended that one builds the dependencies with VS12 as far as
possible, especially those from and using the GTK+ stack (i.e. GDK-Pixbuf,
Pango, GLib and Cairo), so that crashes caused by mixing calls to different
CRTs can be kept at a minimum.

libxml2 and Cairo do contain support for compiling under VS12
using VS project files and/or makefiles at this time of writing.
For GDK-Pixbuf, Pango, libcroco and GLib, VS12 project files are
available under $(srcroot)\build\vs12.

Set up the source tree as follows under some arbitrary top folder
<root>:

<root>\<this-librsvg-source-tree>
<root>\vs12\<PlatformName>

Note: put the resulting Cairo and libxml2 files as follows:
 .dll files: <root>\vs12\<PlatformName>\bin
 .lib files: <root>\vs12\<PlatformName>\lib
 .h files: <root>\vs12\<PlatformName>\include

The recommended build order for these dependencies:
-libxml2
-(optional for GLib) PCRE (version 8.12 or later, use of CMake to
  build PCRE is recommended-see build\win32\vs12\README.txt of GLib)
-GLib (put the sources in <root>\<GLib-Source-Tree>, and build it from
       there with VS12)
-libcroco
-Cairo
-Pango
-Gdk-Pixbuf
-ATK and GTK (Version 3.10+, if building the GTK+ viewer program)

*this* file you are now reading is thus located at
<root>\<this-librsvg-source-tree>\build\win32\vs12\README.txt.

<PlatformName> is either Win32 or x64, as in VS12 project files.

The "install" project will copy build results and headers into their
appropriate location under <root>\vs12\<PlatformName>. For instance,
built DLLs go into <root>\vs12\<PlatformName>\bin, built LIBs into
<root>\vs12\<PlatformName>\lib and headers into
<root>\vs12\<PlatformName>\include\librsvg-2.0. This is then from where
project files higher in the stack are supposed to look for them, not
from a specific librsvg source tree like this one. It is important to
keep separate the concept of a "source tree", where also non-public
headers are present, and an "install tree" where only public headers
are present.

--Updated by Fan, Chun-wei <fanc999@yahoo.com.tw>
