SUBDIRS = . gdk-pixbuf-loader gtk-engine data tests tools doc

lib_LTLIBRARIES = librsvg-@RSVG_API_MAJOR_VERSION@.la

bin_PROGRAMS = rsvg-convert-@RSVG_API_MAJOR_VERSION@
if HAVE_GTK
bin_PROGRAMS += rsvg-view-@RSVG_API_MAJOR_VERSION@
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

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES = 		\
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

librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS = -version-info @RSVG_LT_VERSION_INFO@ -export-dynamic -no-undefined -export-symbols $(srcdir)/librsvg.def
librsvg_@RSVG_API_MAJOR_VERSION@_la_LIBADD = $(LIBGSF_LIBS) $(LIBCROCO_LIBS) $(LIBRSVG_LIBS) -lm

librsvgincdir = $(includedir)/librsvg-$(RSVG_API_VERSION)/librsvg
librsvginc_HEADERS = 	\
	$(headers)	\
	librsvg-features.h \
	librsvg-enum-types.h

INCLUDES =				\
	-I$(top_srcdir) 		\
	-I$(top_builddir) 		\
	$(LIBGSF_CFLAGS)		\
	$(LIBCROCO_CFLAGS)		\
	$(LIBRSVG_CFLAGS) 		\
	$(GTK_CFLAGS)			\
	$(GDK_X11_CFLAGS)		\
	-DG_LOG_DOMAIN=\"librsvg\" 	\
	-DLIBRSVG_DATADIR="\"$(datadir)\""

DEPS = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la
LDADDS =				\
	$(LIBGSF_LIBS)			\
	$(GLIB_LIBS)			\
	$(LIBCROCO_LIBS)		\
	$(LIBRSVG_LIBS) 		\
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la

if GTK_API_VERSION_2
nodist_bin_SCRIPTS = rsvg
man_MANS = rsvg.1
endif

rsvg_convert_@RSVG_API_MAJOR_VERSION@_SOURCES=rsvg-convert.c
rsvg_convert_@RSVG_API_MAJOR_VERSION@_LDFLAGS =
rsvg_convert_@RSVG_API_MAJOR_VERSION@_DEPENDENCIES = $(DEPS)
rsvg_convert_@RSVG_API_MAJOR_VERSION@_LDADD = $(LDADDS) $(libm)

rsvg_view_@RSVG_API_MAJOR_VERSION@_SOURCES = 		\
	test-display.c
rsvg_view_@RSVG_API_MAJOR_VERSION@_LDFLAGS =
rsvg_view_@RSVG_API_MAJOR_VERSION@_DEPENDENCIES = $(DEPS)
rsvg_view_@RSVG_API_MAJOR_VERSION@_LDADD = $(GTK_LIBS) $(GDK_X11_LIBS) $(libm) $(LDADDS)

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
	$(man_MANS)			\
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

if GTK_API_VERSION_2
install-exec-hook:
	@test -e "$(DESTDIR)$(bindir)/rsvg-convert" || (cd $(DESTDIR)$(bindir) && $(LN_S) rsvg-convert-2 rsvg-convert)
	@test -e "$(DESTDIR)$(bindir)/rsvg-view" || (cd $(DESTDIR)$(bindir) && $(LN_S) rsvg-view-2 rsvg-view)
endif
