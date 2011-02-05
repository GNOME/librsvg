SUBDIRS = . gdk-pixbuf-loader data tests tools doc

ACLOCAL_AMFLAGS = ${ACLOCAL_FLAGS}

if ENABLE_GTK_ENGINE
if HAVE_GTK_2
SUBDIRS += gtk-engine
endif
endif

lib_LTLIBRARIES = librsvg-@RSVG_API_MAJOR_VERSION@.la

bin_PROGRAMS = rsvg-convert
if HAVE_GTK_2
bin_PROGRAMS += rsvg-view
endif
if HAVE_GTK_3
bin_PROGRAMS += rsvg-view-3
endif

if OS_WIN32
else
libm = -lm
endif

headers = \
	rsvg.h	\
	rsvg-cairo.h

enum_sources = \
	librsvg-enum-types.h	\
	librsvg-enum-types.c

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES = \
	rsvg-affine.c		\
	librsvg-features.c 	\
	rsvg-bpath-util.c 	\
	rsvg-bpath-util.h 	\
	rsvg-css.c 		\
	rsvg-css.h 		\
	rsvg-defs.c 		\
	rsvg-defs.h 		\
	rsvg-image.c		\
	rsvg-image.h		\
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
	rsvg-xml.c		\
	rsvg-xml.h

librsvg_@RSVG_API_MAJOR_VERSION@_la_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DG_LOG_DOMAIN=\"librsvg\" 		\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_CFLAGS = \
	$(LIBRSVG_CFLAGS) 	\
	$(LIBCROCO_CFLAGS)	\
	$(LIBGSF_CFLAGS)	\
	$(AM_CFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS = -version-info @RSVG_LT_VERSION_INFO@ -export-dynamic -no-undefined -export-symbols $(srcdir)/librsvg.def $(AM_LDFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LIBADD = \
	$(LIBRSVG_LIBS) 	\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS)		\
	$(libm)

librsvgincdir = $(includedir)/librsvg-$(RSVG_API_VERSION)/librsvg
librsvginc_HEADERS = 	\
	$(headers)	\
	librsvg-features.h \
	librsvg-enum-types.h

if HAVE_GTK_2
nodist_bin_SCRIPTS = rsvg
dist_man_MANS = rsvg.1
endif

rsvg_convert_SOURCES = \
	rsvg-convert.c

rsvg_convert_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_convert_CFLAGS =\
	$(LIBRSVG_CFLAGS) 	\
	$(LIBCROCO_CFLAGS)	\
	$(LIBGSF_CFLAGS)	\
	$(AM_CFLAGS)

rsvg_convert_LDFLAGS = $(AM_LDFLAGS)

rsvg_convert_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(LIBRSVG_LIBS)		\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS)		\
	$(GTHREAD_LIBS)		\
	$(libm)

rsvg_view_SOURCES = \
	test-display.c

rsvg_view_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_view_CFLAGS =\
	$(GTK2_CFLAGS)		\
	$(LIBRSVG_CFLAGS) 	\
	$(LIBCROCO_CFLAGS)	\
	$(LIBGSF_CFLAGS)	\
	$(AM_CFLAGS)

rsvg_view_LDFLAGS =

rsvg_view_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK2_LIBS)		\
	$(LIBRSVG_LIBS)		\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS)		\
	$(GTHREAD_LIBS)		\
	$(libm)

rsvg_view_3_SOURCES = \
	test-display.c

rsvg_view_3_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_view_3_CFLAGS =\
	$(GTK3_CFLAGS)		\
	$(LIBRSVG_CFLAGS) 	\
	$(LIBCROCO_CFLAGS)	\
	$(LIBGSF_CFLAGS)	\
	$(AM_CFLAGS)

rsvg_view_3_LDFLAGS =

rsvg_view_3_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK3_LIBS)		\
	$(LIBRSVG_LIBS)		\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS)		\
	$(GTHREAD_LIBS)		\
	$(libm)

EXTRA_DIST =				\
	MAINTAINERS			\
	AUTHORS				\
	NEWS				\
	TODO				\
	COPYING.LIB 			\
	librsvg.spec.in 		\
	librsvg-zip.in 			\
	rsvg.in				\
	librsvg.def			\
	gtk-doc.make

CLEANFILES = \
	$(enum_sources) \
	s-enum-types-h s-enum-types-c

## Put `exec' in the name because this should be installed by
## `install-exec', not `install-data'.

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-$(RSVG_API_VERSION).pc

dist-hook: librsvg.spec
	cp librsvg.spec $(distdir)

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
