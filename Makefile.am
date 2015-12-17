SUBDIRS = . gdk-pixbuf-loader tests tools doc build

NULL =
BUILT_SOURCES =

lib_LTLIBRARIES = librsvg-@RSVG_API_MAJOR_VERSION@.la

bin_PROGRAMS = rsvg-convert
if HAVE_GTK_3
bin_PROGRAMS += rsvg-view-3
endif

headers = \
	rsvg.h	\
	rsvg-cairo.h

enum_sources = \
	librsvg-enum-types.h	\
	librsvg-enum-types.c
BUILT_SOURCES += $(enum_sources)

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES = \
	librsvg-features.c 	\
	rsvg-css.c 		\
	rsvg-css.h 		\
	rsvg-compat.h		\
	rsvg-defs.c 		\
	rsvg-defs.h 		\
	rsvg-image.c		\
	rsvg-image.h		\
	rsvg-io.c		\
	rsvg-io.h		\
	rsvg-paint-server.c 	\
	rsvg-paint-server.h 	\
	rsvg-path.c 		\
	rsvg-path.h 		\
	rsvg-private.h 		\
	rsvg-base-file-util.c 	\
	rsvg-filter.c		\
	rsvg-filter.h		\
	rsvg-marker.c		\
	rsvg-marker.h		\
	rsvg-mask.c		\
	rsvg-mask.h		\
	rsvg-shapes.c		\
	rsvg-shapes.h		\
	rsvg-structure.c	\
	rsvg-structure.h	\
	rsvg-styles.c		\
	rsvg-styles.h		\
	rsvg-text.c		\
	rsvg-text.h		\
	rsvg-cond.c		\
	rsvg-base.c		\
	librsvg-enum-types.c	\
	rsvg-cairo-draw.c	\
	rsvg-cairo-draw.h	\
	rsvg-cairo-render.c	\
	rsvg-cairo-render.h	\
	rsvg-cairo-clip.h	\
	rsvg-cairo-clip.c	\
	rsvg.c			\
	rsvg-gobject.c		\
	rsvg-file-util.c	\
	rsvg-size-callback.c	\
	rsvg-size-callback.h	\
	rsvg-xml.c		\
	rsvg-xml.h		\
	rsvg.h			\
	rsvg-cairo.h		\
	librsvg-features.h 	\
	librsvg-enum-types.h	\
	$(NULL)

librsvg_@RSVG_API_MAJOR_VERSION@_la_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DG_LOG_DOMAIN=\"librsvg\" 		\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	-DRSVG_DISABLE_DEPRECATION_WARNINGS	\
	$(AM_CPPFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_CFLAGS = \
	-DRSVG_COMPILATION	\
	$(LIBRSVG_CFLAGS) 	\
	$(AM_CFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS = \
	$(BSYMBOLIC_LDFLAG) \
	-version-info @RSVG_LT_VERSION_INFO@ \
	-export-dynamic \
	-no-undefined \
	-export-symbols-regex "^rsvg_.*" \
	$(AM_LDFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LIBADD = \
	$(LIBRSVG_LIBS) 	\
	$(LIBM)

librsvgincdir = $(includedir)/librsvg-$(RSVG_API_VERSION)/librsvg
librsvginc_HEADERS = 	\
	$(headers)	\
	librsvg-features.h \
	librsvg-enum-types.h

dist_man_MANS = rsvg-convert.1

rsvg_convert_SOURCES = \
	rsvg-convert.c \
	rsvg-size-callback.c \
	rsvg-size-callback.h \
	$(NULL)

rsvg_convert_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_convert_CFLAGS =\
	$(RSVG_CONVERT_CFLAGS) \
	$(AM_CFLAGS)

rsvg_convert_LDFLAGS = $(AM_LDFLAGS)

rsvg_convert_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(RSVG_CONVERT_LIBS) \
	$(LIBM)

rsvg_view_3_SOURCES = \
	test-display.c \
	rsvg-size-callback.h

rsvg_view_3_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_view_3_CFLAGS =\
	$(GTK3_CFLAGS)		\
	$(LIBRSVG_CFLAGS) 	\
	$(AM_CFLAGS)

rsvg_view_3_LDFLAGS =

rsvg_view_3_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK3_LIBS)		\
	$(LIBRSVG_LIBS)		\
	$(GTHREAD_LIBS)		\
	$(LIBM)

if OS_WIN32
rsvg_view_3_LDFLAGS += -mwindows
endif # OS_WIN32

EXTRA_DIST =				\
	librsvg.doap			\
	AUTHORS				\
	NEWS				\
	TODO				\
	COPYING.LIB 			\
	librsvg-zip.in 			\
	gtk-doc.make			\
	Rsvg-2.0-custom.vala		\
	Rsvg-2.0.metadata		\
	config.h.win32.in		\
	config.h.win32			\
	rsvg.symbols			\
	glib-tap.mk			\
	tap-driver.sh			\
	tap-test

CLEANFILES = \
	$(enum_sources) \
	s-enum-types-h s-enum-types-c

## Put `exec' in the name because this should be installed by
## `install-exec', not `install-data'.

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-$(RSVG_API_VERSION).pc

librsvg-enum-types.h: s-enum-types-h
	@true

s-enum-types-h: $(headers) Makefile
	$(AM_V_GEN) ( cd $(srcdir) && $(GLIB_MKENUMS) \
			--fhead "#if !defined (__RSVG_RSVG_H_INSIDE__) && !defined (RSVG_COMPILATION)\n#warning \"Including <librsvg/librsvg-enum-types.h> directly is deprecated.\"\n#endif\n\n#ifndef __LIBRSVG_ENUM_TYPES_H__\n#define __LIBRSVG_ENUM_TYPES_H__\n\n#include <glib-object.h>\n\nG_BEGIN_DECLS\n" \
			--fprod "/* enumerations from \"@filename@\" */\n" \
			--vhead "GType @enum_name@_get_type (void);\n#define RSVG_TYPE_@ENUMSHORT@ (@enum_name@_get_type())\n" 	\
			--ftail "G_END_DECLS\n\n#endif /* __LIBRSVG_ENUM_TYPES_H__ */" \
		$(headers) ) > tmp-librsvg-enum-types.h \
	&& (cmp -s tmp-librsvg-enum-types.h librsvg-enum-types.h || cp tmp-librsvg-enum-types.h librsvg-enum-types.h ) \
	&& rm -f tmp-librsvg-enum-types.h	\
	&& echo timestamp > $(@F)

librsvg-enum-types.c: s-enum-types-c librsvg-enum-types.h
	@true

s-enum-types-c: $(headers) Makefile
	$(AM_V_GEN) ( cd $(srcdir) && $(GLIB_MKENUMS) \
			--fhead "#include \"rsvg.h\"" \
			--fprod "\n/* enumerations from \"@filename@\" */" \
			--vhead "GType\n@enum_name@_get_type (void)\n{\n  static GType etype = 0;\n  if (etype == 0) {\n    static const G@Type@Value values[] = {" 	\
			--vprod "      { @VALUENAME@, \"@VALUENAME@\", \"@valuenick@\" }," \
			--vtail "      { 0, NULL, NULL }\n    };\n    etype = g_@type@_register_static (\"@EnumName@\", values);\n  }\n  return etype;\n}\n" \
		$(headers) ) > tmp-librsvg-enum-types.c \
	&& (cmp -s tmp-librsvg-enum-types.c librsvg-enum-types.c || cp tmp-librsvg-enum-types.c librsvg-enum-types.c ) \
	&& rm -f tmp-librsvg-enum-types.c \
	&& echo timestamp > $(@F)

DISTCHECK_CONFIGURE_FLAGS = --enable-gtk-doc

if HAVE_INTROSPECTION
-include $(INTROSPECTION_MAKEFILE)

INTROSPECTION_GIRS = Rsvg-@RSVG_API_VERSION@.gir
INTROSPECTION_SCANNER_ARGS = --add-include-path=$(srcdir) --symbol-prefix=rsvg --symbol-prefix=librsvg
INTROSPECTION_COMPILER_ARGS = --includedir=$(srcdir)

Rsvg-@RSVG_API_VERSION@.gir: librsvg-@RSVG_API_MAJOR_VERSION@.la

Rsvg_@RSVG_API_VERSION_U@_gir_NAMESPACE = Rsvg
Rsvg_@RSVG_API_VERSION_U@_gir_EXPORT_PACKAGES = librsvg-$(RSVG_API_VERSION)
Rsvg_@RSVG_API_VERSION_U@_gir_FILES = \
	$(librsvginc_HEADERS) \
	$(filter-out %.h,$(librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES)) \
	$(NULL)
Rsvg_@RSVG_API_VERSION_U@_gir_INCLUDES = \
	GLib-2.0 \
	GObject-2.0 \
	Gio-2.0 \
	cairo-1.0 \
	GdkPixbuf-2.0 \
	$(NULL)
Rsvg_@RSVG_API_VERSION_U@_gir_PACKAGES = \
	pango \
	libxml-2.0 \
	$(NULL)
Rsvg_@RSVG_API_VERSION_U@_gir_CFLAGS = -I$(top_builddir) -DRSVG_COMPILATION
Rsvg_@RSVG_API_VERSION_U@_gir_LIBS = librsvg-@RSVG_API_MAJOR_VERSION@.la
Rsvg_@RSVG_API_VERSION_U@_gir_SCANNERFLAGS = --c-include="librsvg/rsvg.h"

girdir = $(datadir)/gir-1.0
nodist_gir_DATA = $(INTROSPECTION_GIRS)

typelibsdir = $(libdir)/girepository-1.0
nodist_typelibs_DATA = $(INTROSPECTION_GIRS:.gir=.typelib)

CLEANFILES += $(nodist_gir_DATA) $(nodist_typelibs_DATA)

if ENABLE_VAPIGEN
include $(VAPIGEN_MAKEFILE)

librsvg-$(RSVG_API_VERSION).vapi: Rsvg-$(RSVG_API_VERSION).gir

VAPIGEN_VAPIS = librsvg-$(RSVG_API_VERSION).vapi

librsvg_@RSVG_API_VERSION_U@_vapi_DEPS = gio-2.0 cairo
librsvg_@RSVG_API_VERSION_U@_vapi_METADATADIRS = $(srcdir)
librsvg_@RSVG_API_VERSION_U@_vapi_FILES = Rsvg-$(RSVG_API_VERSION).gir Rsvg-$(RSVG_API_VERSION)-custom.vala

vapidir = $(datadir)/vala/vapi
vapi_DATA = $(VAPIGEN_VAPIS)

CLEANFILES += $(VAPIGEN_VAPIS)

endif # ENABLE_VAPIGEN

endif # HAVE_INTROSPECTION

# ------------------- MSVC Build Items ----------------

MSVCPROJS = rsvg rsvg-convert

rsvg_FILES = $(librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES)
rsvg_EXCLUDES = dummy
rsvg_HEADERS_DIR = $(librsvgincdir)
rsvg_HEADERS_INST = $(librsvginc_HEADERS)

rsvg_HEADERS_EXCLUDES = dummy

rsvg_convert_FILES = $(rsvg_convert_SOURCES)
rsvg_convert_EXCLUDES = dummy

include $(top_srcdir)/build/Makefile.msvcproj

# autotools: Why must we do this instead of dist-hook?
MSVCPROJ_GENERATED = \
	$(top_builddir)/build/win32/vs9/rsvg.vcproj		\
	$(top_builddir)/build/win32/vs9/rsvg-convert.vcproj

if HAVE_INTROSPECTION

MSVC_INTROSPECT_GIRS = $(INTROSPECTION_GIRS)

INTROSPECTION_INTERMEDIATE_ITEMS = \
	$(top_builddir)/build/win32/Rsvg-@RSVG_API_VERSION@.gir.msvc.introspect		\
	$(top_builddir)/build/win32/Rsvg_@RSVG_API_VERSION_U@_gir_list

Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_FILES = $(Rsvg_@RSVG_API_VERSION_U@_gir_FILES)
Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_PACKAGES = $(Rsvg_@RSVG_API_VERSION_U@_gir_PACKAGES)
Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_EXPORT_PACKAGES = $(Rsvg_@RSVG_API_VERSION_U@_gir_EXPORT_PACKAGES)
Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_INCLUDE_GIRS = $(Rsvg_@RSVG_API_VERSION_U@_gir_INCLUDES)
Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_LIBS = rsvg-@RSVG_API_VERSION@
Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_CFLAGS = -I../.. -DRSVG_COMPILATION
Rsvg_@RSVG_API_VERSION_U@_gir_MSVC_SCANNERFLAGS = $(Rsvg_@RSVG_API_VERSION_U@_gir_SCANNERFLAGS)

MSVCPROJ_GENERATED += $(INTROSPECTION_INTERMEDIATE_ITEMS)

include $(top_srcdir)/build/Makefile.msvc-introspection
endif # HAVE_INTROSPECTION

$(MSVCPROJ_GENERATED): $(top_builddir)/build/win32/vs9/rsvg.headers

EXTRA_DIST += $(MSVCPROJ_GENERATED)

# ChangeLog generation

ChangeLog:
	$(AM_V_GEN) if test -d $(top_srcdir)/.git; then \
		GIT_DIR="$(top_srcdir)/.git" git log --stat > $@; \
	fi

dist: ChangeLog

.PHONY: ChangeLog
