noinst_PROGRAMS = test-ft-gtk test-ft test-rsvg

EXTRA_DIST =\
	COPYING.LIB \
	librsvg.spec.in \
	librsvg-2.0.pc.in

lib_LTLIBRARIES = librsvg-2.la

librsvg_2_la_SOURCES = \
	art_render.c \
	art_render_gradient.c \
	art_render_mask.c \
	art_render_svp.c \
	art_rgba.c \
	librsvg-features.c \
	rsvg-bpath-util.c \
	rsvg-css.c \
	rsvg-defs.c \
	rsvg-ft.c \
	rsvg-paint-server.c \
	rsvg-path.c \
	rsvg.c

librsvg_2_la_LDFLAGS = -version-info @LIBRSVG_VERSION_INFO@

librsvgincdir = $(includedir)/librsvg-2/librsvg
librsvginc_HEADERS = \
	art_render.h \
	art_render_gradient.h \
	art_render_mask.h \
	art_render_svp.h \
	art_rgba.h \
	rsvg-bpath-util.h \
	rsvg-css.h \
	rsvg-defs.h \
	rsvg-ft.h \
	rsvg-paint-server.h \
	rsvg-path.h \
	rsvg.h \
	librsvg-features.h

INCLUDES =\
	-I$(top_srcdir) \
	-I$(top_builddir) \
	$(FREETYPE2_CFLAGS) \
	$(GLIB_CFLAGS) \
	$(GDK_PIXBUF_CFLAGS) \
	$(GTK_CFLAGS) \
	$(XML_CFLAGS) \
	$(ART_CFLAGS) \
	-DG_LOG_DOMAIN=\"librsvg\" \
	-DG_DISABLE_DEPRECATED \
	-DGDK_DISABLE_DEPRECATED \
	-DGTK_DISABLE_DEPRECATED \
	-DDATADIR="\"$(datadir)\""

DEPS = $(top_builddir)/librsvg-2.la
LDADDS = $(top_builddir)/librsvg-2.la \
	$(XML_LIBS) \
	$(FREETYPE2_LIBS) \
	$(GDK_PIXBUF_LIBS) \
	$(GTK_LIBS) \
	$(GLIB_LIBS) \
	$(FREETYPE2_LIBS) \
	$(ART_LIBS) \
	$(POPT_LIBS) \
	$(LIBPNG)

test_ft_gtk_SOURCES=test-ft-gtk.c
test_ft_gtk_LDFLAGS =
test_ft_gtk_DEPENDENCIES = $(DEPS)
test_ft_gtk_LDADD = $(LDADDS) -lm

test_ft_SOURCES=test-ft.c
test_ft_LDFLAGS =
test_ft_DEPENDENCIES = $(DEPS)
test_ft_LDADD = $(LDADDS) -lm

test_rsvg_SOURCES=test-rsvg.c
test_rsvg_LDFLAGS =
test_rsvg_DEPENDENCIES = $(DEPS)
test_rsvg_LDADD = $(LDADDS) -lm

tests:	test-ft-gtk test-ft test-rsvg

## Put `exec' in the name because this should be installed by
## `install-exec', not `install-data'.

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-2.0.pc

dist-hook: librsvg.spec
	cp librsvg.spec $(distdir)
