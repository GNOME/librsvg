AC_PREREQ([2.69])

# Package version, the "human readable" version
m4_define([rsvg_major_version],[2])
m4_define([rsvg_minor_version],[54])
m4_define([rsvg_micro_version],[7]) # Keep this in sync with Cargo.toml and doc/librsvg.toml
m4_define([rsvg_version],[rsvg_major_version.rsvg_minor_version.rsvg_micro_version])

# Library version information.  To make a release:
#
# - rsvg_lt_revision += 1
#
# - Added interfaces?
#     rsvg_lt_current += 1
#     rsvg_lt_revision = 0
#     rsvg_lt_age += 1
#
# - Removed/changed interfaces?
#     Don't do that! (except for a major release, and only extra-carefully)
#     rsvg_lt_current += 1
#     rsvg_lt_revision = 0
#     rsvg_lt_age = 0
m4_define([rsvg_lt_current],[50])
m4_define([rsvg_lt_revision],[0])
m4_define([rsvg_lt_age],[48])
m4_define([rsvg_lt_version_info],rsvg_lt_current:rsvg_lt_revision:rsvg_lt_age)

AC_INIT([RSVG],[rsvg_version],[https://gitlab.gnome.org/GNOME/librsvg/issues],[librsvg])

AC_CONFIG_HEADERS([config.h])
AC_CONFIG_SRCDIR([include/librsvg/rsvg.h])
AC_CONFIG_MACRO_DIR([m4])

AM_INIT_AUTOMAKE([1.9 foreign no-dist-gzip dist-xz tar-ustar subdir-objects])
m4_ifdef([AM_SILENT_RULES], [AM_SILENT_RULES([yes])])

AM_MAINTAINER_MODE([enable])

dnl ===========================================================================
dnl API versioning
dnl ===========================================================================

RSVG_API_VERSION=2.0
RSVG_API_MAJOR_VERSION=2
RSVG_API_MINOR_VERSION=0

AC_SUBST([RSVG_API_VERSION])
AC_SUBST([RSVG_API_MAJOR_VERSION])
AC_SUBST([RSVG_API_MINOR_VERSION])
AC_SUBST([RSVG_API_VERSION_U],[AS_TR_SH([$RSVG_API_VERSION])])

# ===========================================================================

AC_SUBST([RSVG_LT_VERSION_INFO],[rsvg_lt_version_info])
AC_SUBST([LIBRSVG_MAJOR_VERSION],[rsvg_major_version])
AC_SUBST([LIBRSVG_MINOR_VERSION],[rsvg_minor_version])
AC_SUBST([LIBRSVG_MICRO_VERSION],[rsvg_micro_version])

dnl ===========================================================================
dnl Please update COMPILING.md with version numbers if these change
CAIRO_REQUIRED=1.16.0
dnl This corresponds to Freetype2 2.8
FREETYPE2_REQUIRED=20.0.14
GDK_PIXBUF_REQUIRED=2.20
GIO_REQUIRED=2.24.0
GLIB_REQUIRED=2.50.0
HARFBUZZ_REQUIRED=2.0.0
LIBXML_REQUIRED=2.9.0
PANGO_REQUIRED=1.46.0

dnl ===========================================================================

AC_PROG_MKDIR_P
AC_PROG_LN_S
AC_PROG_CC
AC_PROG_CPP
AC_PROG_AWK
AM_PROG_CC_C_O
# Check if -lcposix is required (for ISC UNIX)
AC_SEARCH_LIBS([strerror], [cposix])

AC_C_BIGENDIAN

PKG_PROG_PKG_CONFIG
AS_IF([test -z "$PKG_CONFIG"],
  [AC_MSG_WARN([pkg-config is needed for auto-configuration of dependencies])
   PKG_CONFIG=false])

LT_INIT([win32-dll])
LT_LIB_M

# Check if -ldl is needed to use dlopen()
AC_CHECK_FUNC(dlopen, [],
              [AC_CHECK_LIB(dl, dlopen, [DLOPEN_LIBS=-ldl])])
AC_SUBST(DLOPEN_LIBS)

dnl ===========================================================================

AC_CHECK_TOOL(CARGO, [cargo], [no])
AS_IF(test x$CARGO = xno,
    AC_MSG_ERROR([cargo is required.  Please install the Rust toolchain from https://www.rust-lang.org/])
)
AC_CHECK_TOOL(RUSTC, [rustc], [no])
AS_IF(test x$RUSTC = xno,
    AC_MSG_ERROR([rustc is required.  Please install the Rust toolchain from https://www.rust-lang.org/])
)

dnl MSRV - Minimum Supported Rust Version
dnl If you change this, please update COMPILING.md
MINIMUM_RUST_VER=1.56
rust_version=`$RUSTC --version | sed -e 's/^rustc //g'`
AX_COMPARE_VERSION([$rust_version],[lt],[$MINIMUM_RUST_VER], [
    AC_MSG_ERROR([librsvg needs at least rustc version $MINIMUM_RUST_VER])
])

dnl Man page generator

AC_CHECK_PROGS(RST2MAN, [rst2man rst2man.py], [no])
AS_IF(test x$RST2MAN = xno,
    AC_MSG_WARN([rst2man not found - not building man pages])
)
AM_CONDITIONAL(HAVE_RST2MAN, [test "x$RST2MAN" != "xno"])

dnl Gi-docgen

AC_ARG_ENABLE([gtk-doc],
              [AS_HELP_STRING([--disable-gtk-doc],[Disable building documentation (requires introspection)])],
              [enable_gtk_doc=$enableval],[enable_gtk_doc=auto])

AS_IF([test "x$enable_gtk_doc" != xno],
      [AC_CHECK_TOOL(GI_DOCGEN, [gi-docgen], [no])
       AS_IF([test "x$GI_DOCGEN" = xno],
             [AS_CASE(["$enable_gtk_doc"],
                      [yes], [AC_MSG_ERROR([gi-docgen not found - cannot build docs])],
                      [auto], [AC_MSG_WARN([gi-docgen not found - not building docs])])
              enable_gtk_doc=no],
             [enable_gtk_doc=yes])])
AM_CONDITIONAL(ENABLE_GTK_DOC, [test "x$enable_gtk_doc" = xyes])

dnl ===========================================================================

#AC_CANONICAL_HOST

AC_MSG_CHECKING([platform])
platform=POSIX
gio_backend=unix
AS_CASE(["$host"],
  [*-*-mingw*],  [platform=Windows gio_backend=windows],
  [*-*-cygwin*], [platform=Cygwin],
  [*-*-darwin*], [platform=Darwin],
)
AC_MSG_RESULT([$platform])
AM_CONDITIONAL(PLATFORM_WIN32,
    [test x"$platform" = xWindows || test x"$platform" = xCygwin])
AM_CONDITIONAL(OS_WIN32,  [test x"$platform" = xWindows])
AM_CONDITIONAL(OS_DARWIN, [test x"$platform" = xDarwin])

dnl ===========================================================================

PKG_CHECK_MODULES(LIBRSVG,                     \
	cairo >= $CAIRO_REQUIRED               \
	cairo-png >= $CAIRO_REQUIRED           \
	cairo-gobject >= $CAIRO_REQUIRED       \
	freetype2 >= $FREETYPE2_REQUIRED       \
	gdk-pixbuf-2.0 >= $GDK_PIXBUF_REQUIRED \
	gio-2.0 >= $GIO_REQUIRED               \
	glib-2.0 >= $GLIB_REQUIRED             \
	harfbuzz >= $HARFBUZZ_REQUIRED         \
	libxml-2.0 >= $LIBXML_REQUIRED         \
	pangocairo >= $PANGO_REQUIRED          \
	pangoft2 >= $PANGO_REQUIRED	       \
)

CAIRO_VERSION=`$PKG_CONFIG --modversion cairo`
PANGO_VERSION=`$PKG_CONFIG --modversion pangocairo`
FREETYPE_VERSION=`$PKG_CONFIG --modversion freetype2`
HARFBUZZ_VERSION=`$PKG_CONFIG --modversion harfbuzz`

PKG_CHECK_MODULES([GTHREAD],[gthread-2.0 >= $GLIB_REQUIRED])

PKG_CHECK_MODULES([GMODULE],[gmodule-2.0])

PKG_CHECK_MODULES([RSVG_CONVERT],[gio-2.0 "gio-${gio_backend}-2.0" gdk-pixbuf-2.0 cairo pangocairo])

GLIB_TESTS

AS_IF([test x"$platform" = xWindows], [
  ## Cannot use AC_CHECK_LIB to check for function with __stdcall
  ## calling convention.  Instead just check for the header and add the
  ## corresponding library to the linker flags.
  AC_CHECK_HEADER([winsock2.h], [WS2_32_LIBS=-lws2_32], [], [#include <windows.h>])
  LIBS="$LIBS $WS2_32_LIBS"
  AC_CHECK_HEADER([userenv.h], [USERENV_LIBS=-luserenv], [], [#include <windows.h>])
  LIBS="$LIBS $USERENV_LIBS"
  AC_CHECK_HEADER([bcrypt.h], [BCRYPT_LIBS=-lbcrypt], [], [#include <windows.h>])
  LIBS="$LIBS $BCRYPT_LIBS"
])

dnl ===========================================================================
dnl GDK-Pixbuf SVG loader
dnl ===========================================================================

AC_MSG_CHECKING([whether to build the GDK-Pixbuf SVG loader])
AC_ARG_ENABLE([pixbuf-loader],
  [AS_HELP_STRING([--disable-pixbuf-loader],[Disable a RSVG based GdkPixbuf loader (default=yes)])],
  [],[enable_pixbuf_loader=yes])
AC_MSG_RESULT([$enable_pixbuf_loader])

gdk_pixbuf_moduledir=
AS_IF([test "x$enable_pixbuf_loader" = "xyes"], [
  PKG_CHECK_MODULES([GDK_PIXBUF],[gdk-pixbuf-2.0 >= $GDK_PIXBUF_REQUIRED])

  AC_PATH_PROGS([GDK_PIXBUF_QUERYLOADERS],[gdk-pixbuf-query-loaders gdk-pixbuf-query-loaders-64 gdk-pixbuf-query-loaders-32],[none])

  AS_IF([test "x$GDK_PIXBUF_QUERYLOADERS" = "xnone"], [
    AC_MSG_ERROR([gdk-pixbuf-query-loaders not found in path])
  ])

  # First try the standalone gdk-pixbuf
  gdk_pixbuf_binary_version="`$PKG_CONFIG --variable=gdk_pixbuf_binary_version gdk-pixbuf-2.0`"
  gdk_pixbuf_binarydir="`$PKG_CONFIG --variable=gdk_pixbuf_binarydir gdk-pixbuf-2.0`"
  gdk_pixbuf_moduledir=`$PKG_CONFIG --variable gdk_pixbuf_moduledir gdk-pixbuf-2.0`
  gdk_pixbuf_cache_file=`$PKG_CONFIG --variable gdk_pixbuf_cache_file gdk-pixbuf-2.0`
])

AC_SUBST([gdk_pixbuf_binary_version])
AC_SUBST([gdk_pixbuf_binarydir])
AC_SUBST([gdk_pixbuf_moduledir])
AC_SUBST([gdk_pixbuf_cache_file])

AM_CONDITIONAL([ENABLE_PIXBUF_LOADER],[test "$enable_pixbuf_loader" = "yes"])

##########################################################
# Check for -Bsymbolic-functions linker flag used to avoid
# intra-library PLT jumps, if available.
##########################################################

AC_ARG_ENABLE([Bsymbolic],
  [AS_HELP_STRING([--disable-Bsymbolic],
                  [disable linking with -Bsymbolic])],
  [enable_Bsymbolic=no],[enable_Bsymbolic=auto])

BSYMBOLIC_LDFLAG=
AS_IF([test "x$enable_Bsymbolic" != "xno"], [
  CC_CHECK_LDFLAGS(["-Wl,-Bsymbolic-functions"],
    [BSYMBOLIC_LDFLAG="-Wl,-Bsymbolic-functions"],
    [AS_IF([test "x$enable_Bsymbolic" = "xauto"], [
       AC_MSG_WARN([-Bsymbolic not supported by ld; disabling])
       enable_Bsymbolic=no
     ], [
       AC_MSG_ERROR([-Bsymbolic requested but not supported by ld. Use --disable-Bsymbolic to disable])
     ])])
])

AC_SUBST([BSYMBOLIC_LDFLAG])

AC_SUBST([GLIB_PREFIX],[$($PKG_CONFIG --variable=prefix glib-2.0)])
AC_SUBST([GDK_PIXBUF_PREFIX],[$($PKG_CONFIG --variable=prefix gdk-pixbuf-2.0)])
AC_SUBST([GTK_PREFIX],[$($PKG_CONFIG --variable=prefix gdk-pixbuf-2.0)])
AC_SUBST([CAIRO_PREFIX],[$($PKG_CONFIG --variable=prefix cairo)])

dnl ===========================================================================
dnl Specify --enable-debug to make a development release.  By default,
dnl we build in public release mode.

AC_ARG_ENABLE(debug,
              AS_HELP_STRING([--enable-debug],
                             [Build Rust code with debugging information [default=no]]),
              [debug_release=$enableval],
	      [AS_CASE(["$LIBRSVG_DEBUG"],
		[yes], [debug_release=yes],
		[no|''], [debug_release=no])
	      ])

AC_MSG_CHECKING(whether to build Rust code with debugging information)
AS_IF([test "x$debug_release" = "xyes"], [
	AC_MSG_RESULT(yes)
	RUST_TARGET_SUBDIR=debug
], [
	AC_MSG_RESULT(no)
	RUST_TARGET_SUBDIR=release
])
AM_CONDITIONAL([DEBUG_RELEASE], [test "x$debug_release" = "xyes"])

AC_ARG_VAR(RUST_TARGET, [Set rust target (default=$host)])
: ${RUST_TARGET:=$host}

AM_CONDITIONAL([CROSS_COMPILING], [test "x$cross_compiling" = xyes])
AS_IF([test "x$cross_compiling" = "xyes"], [
	RUST_TARGET_SUBDIR="$RUST_TARGET/$RUST_TARGET_SUBDIR"
])
AC_SUBST([RUST_TARGET_SUBDIR])

dnl ===========================================================================
dnl Build introspectable bindings

# No automagic please!
: ${enable_introspection:=yes}

GOBJECT_INTROSPECTION_CHECK([0.10.8])

# Vala bindings
VAPIGEN_CHECK([0.17.1.26],,,[no])

dnl ===========================================================================

m4_copy([AC_DEFUN],[glib_DEFUN])
glib_DEFUN([GLIB_LC_MESSAGES],
  [AC_CHECK_HEADERS([locale.h])
  AS_IF([test "x$ac_cv_header_locale_h" = xyes], [
    AC_CACHE_CHECK([for LC_MESSAGES], am_cv_val_LC_MESSAGES,
      [AC_LINK_IFELSE([AC_LANG_PROGRAM([[#include <locale.h>]],
          [[return LC_MESSAGES]])],
	[am_cv_val_LC_MESSAGES=yes],
	[am_cv_val_LC_MESSAGES=no])])
    AS_IF([test "x$am_cv_val_LC_MESSAGES" = xyes], [
      AC_DEFINE(HAVE_LC_MESSAGES, 1,
        [Define if your <locale.h> file defines LC_MESSAGES.])
  ])])])

GLIB_LC_MESSAGES

dnl ===========================================================================

# Check whether MSVC toolset is explicitly set

AM_CONDITIONAL(MSVC_BASE_NO_TOOLSET_SET, [test x$MSVC_BASE_TOOLSET = x])
AM_CONDITIONAL(MSVC_NO_TOOLSET_SET, [test x$MSVC_TOOLSET = x])

dnl ===========================================================================

AC_SUBST([AM_CPPFLAGS])
AC_SUBST([AM_CFLAGS])
AC_SUBST([AM_LDFLAGS])

AC_CONFIG_FILES([
include/librsvg/rsvg-version.h
Makefile
librsvg-zip
gdk-pixbuf-loader/Makefile
tests/Makefile
doc/Makefile
win32/Makefile
win32/config.h.win32
win32/config-msvc.mak
])

AC_CONFIG_FILES([librsvg-${RSVG_API_VERSION}.pc:librsvg.pc.in],[],[RSVG_API_VERSION=$RSVG_API_VERSION])

AC_OUTPUT

dnl =============================================================================================
echo "
librsvg-$VERSION

	prefix:                         ${prefix}
	C compiler:                     ${CC}
	rustc:                          rustc ${rust_version}

	Cairo version:                  ${CAIRO_VERSION}
	Pango version:                  ${PANGO_VERSION}
	Freetype version:               ${FREETYPE_VERSION}
	Harfbuzz version:               ${HARFBUZZ_VERSION}

        Debugging information for Rust: ${debug_release}

	Build introspectable bindings:  ${found_introspection}
	Build Vala bindings:            ${enable_vala}
	Build GdkPixbuf loader:         ${enable_pixbuf_loader}
	Build developer documentation:  ${enable_gtk_doc}
"
