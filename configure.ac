m4_define([rsvg_major_version],[2])
m4_define([rsvg_minor_version],[44])
m4_define([rsvg_micro_version],[10])
m4_define([rsvg_extra_version],[])
m4_define([rsvg_version],[rsvg_major_version.rsvg_minor_version.rsvg_micro_version()rsvg_extra_version])
m4_define([rsvg_lt_version_info],m4_eval(rsvg_major_version + rsvg_minor_version):rsvg_micro_version:rsvg_minor_version)

AC_INIT([RSVG],[rsvg_version],[https://gitlab.gnome.org/GNOME/librsvg/issues],[librsvg])

AC_CONFIG_HEADERS([config.h])
AC_CONFIG_SRCDIR([librsvg/rsvg.h])
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
CAIRO_REQUIRED=1.15.12
CROCO_REQUIRED=0.6.1
dnl This corresponds to Freetype2 2.8
FREETYPE2_REQUIRED=20.0.14
GDK_PIXBUF_REQUIRED=2.20
GIO_REQUIRED=2.24.0
GLIB_REQUIRED=2.48.0
GTK3_REQUIRED=3.10.0
LIBXML_REQUIRED=2.9.0
PANGO_REQUIRED=1.38.0

dnl ===========================================================================

AC_PROG_LN_S
AC_ISC_POSIX
AC_PROG_CC
AC_PROG_CPP
AC_PROG_AWK
AC_STDC_HEADERS
AM_PROG_CC_C_O

AC_C_BIGENDIAN

PKG_PROG_PKG_CONFIG

LT_INIT([win32-dll])
LT_LIB_M

# Check if -ldl is needed to use dlopen()
AC_CHECK_FUNC(dlopen, [],
              [AC_CHECK_LIB(dl, dlopen, [DLOPEN_LIBS=-ldl])])
AC_SUBST(DLOPEN_LIBS)

dnl ===========================================================================

AC_CHECK_PROG(CARGO, [cargo], [yes], [no])
AS_IF(test x$CARGO = xno,
    AC_MSG_ERROR([cargo is required.  Please install the Rust toolchain from https://www.rust-lang.org/])
)
AC_CHECK_PROG(RUSTC, [rustc], [yes], [no])
AS_IF(test x$RUSTC = xno,
    AC_MSG_ERROR([rustc is required.  Please install the Rust toolchain from https://www.rust-lang.org/])
)

dnl Minimum version of rustc that we support
dnl If you change this, please update COMPILING.md
MINIMUM_RUST_MAJOR=1
MINIMUM_RUST_MINOR=27

rust_version=`rustc --version`
version=`echo "$rust_version" | sed -e 's/^rustc //g'`
major=`echo "$version" | cut -d. -f1`
minor=`echo "$version" | cut -d. -f2`

AS_IF(test "$major" -lt "$MINIMUM_RUST_MAJOR" -o "$minor" -lt "$MINIMUM_RUST_MINOR",
    AC_MSG_ERROR([librsvg needs at rustc version at least $MINIMUM_RUST_MAJOR.$MINIMUM_RUST_MINOR])
)

dnl ===========================================================================

#AC_CANONICAL_HOST

AC_MSG_CHECKING([for some Win32 platform])
case "$host" in
  *-*-mingw*|*-*-cygwin*)
    platform_win32=yes
    ;;
  *)
    platform_win32=no
    ;;
esac
AC_MSG_RESULT([$platform_win32])
AM_CONDITIONAL(PLATFORM_WIN32, test "$platform_win32" = "yes")

AC_MSG_CHECKING([for native Win32])
case "$host" in
  *-*-mingw*)
    native_win32=yes
    ;;
  *)
    native_win32=no
    ;;
esac
AC_MSG_RESULT([$native_win32])
AM_CONDITIONAL(OS_WIN32, test "$native_win32" = "yes")

dnl ===========================================================================

GLIB_MKENUMS=`$PKG_CONFIG --variable=glib_mkenums glib-2.0`
AC_SUBST(GLIB_MKENUMS)

PKG_CHECK_MODULES(LIBRSVG,                     \
	cairo >= $CAIRO_REQUIRED               \
	cairo-png >= $CAIRO_REQUIRED           \
	freetype2 >= $FREETYPE2_REQUIRED       \
	gdk-pixbuf-2.0 >= $GDK_PIXBUF_REQUIRED \
	gio-2.0 >= $GIO_REQUIRED               \
	glib-2.0 >= $GLIB_REQUIRED             \
	libxml-2.0 >= $LIBXML_REQUIRED         \
	pangocairo >= $PANGO_REQUIRED          \
	pangoft2 >= $PANGO_REQUIRED            \
        libcroco-0.6 >= $CROCO_REQUIRED)

PKG_CHECK_MODULES([GTHREAD],[gthread-2.0 >= $GLIB_REQUIRED])

PKG_CHECK_MODULES([GMODULE],[gmodule-2.0])

case "$host" in
  *-*-mingw*)
    PKG_CHECK_MODULES([RSVG_CONVERT],[gio-2.0 gio-windows-2.0 gdk-pixbuf-2.0 cairo pangocairo])
    ;;
  *)
    PKG_CHECK_MODULES([RSVG_CONVERT],[gio-2.0 gio-unix-2.0 gdk-pixbuf-2.0 cairo pangocairo])
    ;;
esac

GLIB_TESTS

# ===========================================================================
# GTK
# ===========================================================================

PKG_CHECK_MODULES([GTK3],[gtk+-3.0 >= $GTK3_REQUIRED],[have_gtk_3=yes],[have_gtk_3=no])

GTK3_BINARY_VERSION=

if test "$have_gtk_3" = "yes"; then
    GTK3_BINARY_VERSION="`$PKG_CONFIG --variable=gtk_binary_version gtk+-3.0`"
fi

AC_SUBST([GTK3_BINARY_VERSION])

AM_CONDITIONAL([HAVE_GTK_3],[test "$have_gtk_3" = "yes"])

dnl ===========================================================================
dnl GDK-Pixbuf SVG loader
dnl ===========================================================================

AC_MSG_CHECKING([whether to build the GDK-Pixbuf SVG loader])
AC_ARG_ENABLE([pixbuf-loader],
  [AS_HELP_STRING([--disable-pixbuf-loader],[Disable a RSVG based GdkPixbuf loader (default=yes)])],
  [],[enable_pixbuf_loader=yes])
AC_MSG_RESULT([$enable_pixbuf_loader])

gdk_pixbuf_moduledir=
if test "$enable_pixbuf_loader" = "yes"; then
  PKG_CHECK_MODULES([GDK_PIXBUF],[gdk-pixbuf-2.0 >= $GDK_PIXBUF_REQUIRED])

  AC_PATH_PROGS([GDK_PIXBUF_QUERYLOADERS],[gdk-pixbuf-query-loaders gdk-pixbuf-query-loaders-64 gdk-pixbuf-query-loaders-32],[none])

  if test "x$GDK_PIXBUF_QUERYLOADERS" = "xnone"; then
    AC_MSG_ERROR([gdk-pixbuf-query-loaders not found in path])
  fi

  # First try the standalone gdk-pixbuf
  gdk_pixbuf_binary_version="`$PKG_CONFIG --variable=gdk_pixbuf_binary_version gdk-pixbuf-2.0`"
  gdk_pixbuf_binarydir="`$PKG_CONFIG --variable=gdk_pixbuf_binarydir gdk-pixbuf-2.0`"
  gdk_pixbuf_moduledir=`$PKG_CONFIG --variable gdk_pixbuf_moduledir gdk-pixbuf-2.0`
  gdk_pixbuf_cache_file=`$PKG_CONFIG --variable gdk_pixbuf_cache_file gdk-pixbuf-2.0`
fi

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
if test "$enable_Bsymbolic" != "no"; then
  CC_CHECK_LDFLAGS(["-Wl,-Bsymbolic-functions"],
    [BSYMBOLIC_LDFLAG="-Wl,-Bsymbolic-functions"],
    [if test "$enable_Bsymbolic" = "auto"; then
       AC_MSG_WARN([-Bsymbolic not supported by ld; disabling])
       enable_Bsymbolic=no
     else
       AC_MSG_ERROR([-Bsymbolic requested but not supported by ld. Use --disable-Bsymbolic to disable])
     fi])
fi

AC_SUBST([BSYMBOLIC_LDFLAG])

##################################################
# Checks for gtk-doc and docbook-tools
##################################################

GTK_DOC_CHECK([1.13],[--flavour no-tmpl])

AC_SUBST([GLIB_PREFIX],[$($PKG_CONFIG --variable=prefix glib-2.0)])
AC_SUBST([GDK_PIXBUF_PREFIX],[$($PKG_CONFIG --variable=prefix gdk-pixbuf-2.0)])
AC_SUBST([GTK_PREFIX],[$($PKG_CONFIG --variable=prefix gdk-pixbuf-2.0)])
AC_SUBST([CAIRO_PREFIX],[$($PKG_CONFIG --variable=prefix cairo)])

dnl ===========================================================================
dnl Do we build the miscellaneous tools provided with librsvg ?

AC_ARG_ENABLE(tools,
              AC_HELP_STRING([--disable-tools],
                             [do not build miscellaneous tools [default=no]]),
              [build_misc_tools=$enableval],
              [build_misc_tools=yes])

AC_MSG_CHECKING(whether to build miscellaneous tools)
if test "x$build_misc_tools" = "xyes" ; then
	AC_MSG_RESULT(yes)
else
	AC_MSG_RESULT(no)
fi
AM_CONDITIONAL([BUILD_MISC_TOOLS], [test "x$build_misc_tools" = "xyes"])

dnl ===========================================================================
dnl Specify --enable-debug to make a development release.  By default,
dnl we build in public release mode.

AC_ARG_ENABLE(debug,
              AC_HELP_STRING([--enable-debug],
                             [Build Rust code with debugging information [default=no]]),
              [debug_release=$enableval],
	      [
	        if test "x$LIBRSVG_DEBUG" == "x"; then
		  debug_release="no"
		elif test "x$LIBRSVG_DEBUG" = "xyes"; then
		  debug_release="yes"
		elif test "x$LIBRSVG_DEBUG" = "xno"; then
		  debug_release="no"
		fi
	      ])

AC_MSG_CHECKING(whether to build Rust code with debugging information)
if test "x$debug_release" = "xyes" ; then
	AC_MSG_RESULT(yes)
	RUST_TARGET_SUBDIR=debug
else
	AC_MSG_RESULT(no)
	RUST_TARGET_SUBDIR=release
fi
AM_CONDITIONAL([DEBUG_RELEASE], [test "x$debug_release" = "xyes"])

AC_ARG_VAR(RUST_TARGET, [Set rust target (default=$host)])
if test "x$RUST_TARGET" == x; then
	RUST_TARGET=$host
fi

AM_CONDITIONAL(CROSS_COMPILING, test $cross_compiling = yes)
if test "x$cross_compiling" = "xyes" ; then
	RUST_TARGET_SUBDIR="$RUST_TARGET/$RUST_TARGET_SUBDIR"
fi
AC_SUBST([RUST_TARGET_SUBDIR])

dnl ===========================================================================
dnl Build introspectable bindings

# No automagic please!
if test -z "$enable_introspection"; then
  enable_introspection=yes
fi

GOBJECT_INTROSPECTION_CHECK([0.10.8])

# Vala bindings
VAPIGEN_CHECK([0.17.1.26],,,[no])

dnl ===========================================================================

m4_copy([AC_DEFUN],[glib_DEFUN])
glib_DEFUN([GLIB_LC_MESSAGES],
  [AC_CHECK_HEADERS([locale.h])
    if test $ac_cv_header_locale_h = yes; then
    AC_CACHE_CHECK([for LC_MESSAGES], am_cv_val_LC_MESSAGES,
      [AC_TRY_LINK([#include <locale.h>], [return LC_MESSAGES],
       am_cv_val_LC_MESSAGES=yes, am_cv_val_LC_MESSAGES=no)])
    if test $am_cv_val_LC_MESSAGES = yes; then
      AC_DEFINE(HAVE_LC_MESSAGES, 1,
        [Define if your <locale.h> file defines LC_MESSAGES.])
    fi
  fi])

GLIB_LC_MESSAGES

dnl ===========================================================================

# Check for pangoft2 and fontconfig, which is optional on Windows

PKG_CHECK_MODULES([PANGOFT2],[pangoft2 >= $PANGO_REQUIRED],[have_pangoft2=yes],[have_pangoft2=no])
PKG_CHECK_MODULES([FONTCONFIG],[fontconfig],[have_fontconfig=yes],[have_fontconfig=no])

if test "x$native_win32" != "xyes"; then
  if test "x$have_pangoft2" != "xyes" -o "x$have_fontconfig" != "xyes"; then
    AC_MSG_ERROR([pangoft2 and fontconfig are required for non-Windows platforms.])
  fi
fi

if test "x$have_pangoft2" = "xyes" -a "x$have_fontconfig" = "xyes"; then
  AC_DEFINE(HAVE_PANGOFT2, 1, [Have the pangoft2 library])
fi

dnl ===========================================================================

# Check whether MSVC toolset is explicitly set

AM_CONDITIONAL(MSVC_BASE_NO_TOOLSET_SET, [test x$MSVC_BASE_TOOLSET = x])
AM_CONDITIONAL(MSVC_NO_TOOLSET_SET, [test x$MSVC_TOOLSET = x])

dnl ===========================================================================

AC_SUBST([AM_CPPFLAGS])
AC_SUBST([AM_CFLAGS])
AC_SUBST([AM_LDFLAGS])

AC_CONFIG_FILES([
librsvg/librsvg-features.h
Makefile
librsvg-zip
gdk-pixbuf-loader/Makefile
tests/Makefile
tools/Makefile
doc/Makefile
doc/version.xml
config.h.win32
win32/Makefile
win32/vs12/Makefile
win32/vs12/rsvg-version-paths.props
win32/vs14/Makefile
win32/vs15/Makefile
])

AC_CONFIG_FILES([librsvg-${RSVG_API_VERSION}.pc:librsvg.pc.in],[],[RSVG_API_VERSION=$RSVG_API_VERSION])
AC_CONFIG_FILES([librsvg-${RSVG_API_VERSION}-uninstalled.pc:librsvg-uninstalled.pc.in],[],[RSVG_API_VERSION=$RSVG_API_VERSION])

AC_OUTPUT

dnl =============================================================================================
echo "
librsvg-$VERSION

	prefix:                         ${prefix}
	C compiler:                     ${CC}
	rustc:                          ${rust_version}

        Debugging information for Rust: ${debug_release}

	Build introspectable bindings:  ${found_introspection}
	Build Vala bindings:            ${enable_vala}
	Build GdkPixbuf loader:         ${enable_pixbuf_loader}
        GTK+ $GTK3_REQUIRED or later:           ${have_gtk_3}
	Build miscellaneous tools:      ${build_misc_tools}
	Build HTML documentation:       ${enable_gtk_doc}
"
