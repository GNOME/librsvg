SUBDIRS = . gdk-pixbuf-loader tests tools doc

NULL =

ACLOCAL_AMFLAGS = ${ACLOCAL_FLAGS}

if ENABLE_GTK2_ENGINE
SUBDIRS += gtk-engine
endif

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

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES = \
	librsvg-features.c 	\
	rsvg-css.c 		\
	rsvg-css.h 		\
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
	$(LIBRSVG_CFLAGS) 	\
	$(AM_CFLAGS)

rsvg_convert_LDFLAGS = $(AM_LDFLAGS)

rsvg_convert_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(LIBRSVG_LIBS)		\
	$(GTHREAD_LIBS)		\
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
	gtk-doc.make

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
			--fhead "#ifndef __LIBRSVG_ENUM_TYPES_H__\n#define __LIBRSVG_ENUM_TYPES_H__\n\n#include <glib-object.h>\n\nG_BEGIN_DECLS\n" \
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
			--fhead "#include \"librsvg-enum-types.h\"\n#include \"rsvg.h\"" \
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
INTROSPECTION_SCANNER_ARGS = --add-include-path=$(srcdir)
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
Rsvg_@RSVG_API_VERSION_U@_gir_CFLAGS = -I$(top_builddir)
Rsvg_@RSVG_API_VERSION_U@_gir_LIBS = librsvg-@RSVG_API_MAJOR_VERSION@.la

girdir = $(datadir)/gir-1.0
nodist_gir_DATA = $(INTROSPECTION_GIRS)

typelibsdir = $(libdir)/girepository-1.0
nodist_typelibs_DATA = $(INTROSPECTION_GIRS:.gir=.typelib)

CLEANFILES += $(nodist_gir_DATA) $(nodist_typelibs_DATA)

endif # HAVE_INTROSPECTION

# ChangeLog generation

ChangeLog:
	$(AM_V_GEN) if test -d $(top_srcdir)/.git; then \
		GIT_DIR="$(top_srcdir)/.git" git log --stat > $@; \
	fi

dist: ChangeLog

.PHONY: ChangeLog
