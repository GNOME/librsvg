SUBDIRS = . gtk-engine gdk-pixbuf-loader moz-plugin doc data

if HAVE_GTK
target_rsvg_view = rsvg-view
else
target_rsvg_view =
endif

bin_PROGRAMS = rsvg $(target_rsvg_view)

noinst_PROGRAMS = test-performance

man_MANS = rsvg.1

if OS_WIN32
else
libm = -lm
endif

lib_LTLIBRARIES = librsvg-2.la

librsvg_2_la_SOURCES = 		\
	rsvg-gz.c		\
	librsvg-features.c 	\
	rsvg-bpath-util.c 	\
	rsvg-bpath-util.h 	\
	rsvg-css.c 		\
	rsvg-css.h 		\
	rsvg-defs.c 		\
	rsvg-defs.h 		\
	rsvg-paint-server.c 	\
	rsvg-paint-server.h 	\
	rsvg-path.c 		\
	rsvg-path.h 		\
	rsvg-private.h 		\
	rsvg-file-util.c 	\
	rsvg-filter.c		\
	rsvg-filter.h		\
	rsvg-mask.c		\
	rsvg-mask.h		\
	rsvg-shapes.c		\
	rsvg-shapes.h		\
	rsvg-styles.c		\
	rsvg-styles.h		\
	rsvg-text.c		\
	rsvg-text-vectors.c 	\
	rsvg-text.h		\
	rsvg.c

librsvg_2_la_LDFLAGS = -version-info @VERSION_INFO@ -no-undefined -export-dynamic
librsvg_2_la_LIBADD = $(LIBRSVG_LIBS) $(LIBGSF_LIBS) $(LIBCROCO_LIBS)

librsvgincdir = $(includedir)/librsvg-2/librsvg
librsvginc_HEADERS = 	\
	rsvg-gz.h	\
	rsvg.h 		\
	librsvg-features.h

INCLUDES =				\
	$(LIBGSF_CFLAGS)		\
	$(LIBCROCO_CFLAGS)		\
	-I$(top_srcdir) 		\
	-I$(top_builddir) 		\
	$(LIBRSVG_CFLAGS) 		\
	$(GTK_CFLAGS)			\
	$(GDK_X11_CFLAGS)		\
	-DG_LOG_DOMAIN=\"librsvg\" 	\
	-DDATADIR="\"$(datadir)\""	\
	-DPIXMAPDIR=\""$(datadir)/pixmaps"\"

DEPS = $(top_builddir)/librsvg-2.la
LDADDS =				\
	$(LIBGSF_LIBS)			\
	$(LIBCROCO_LIBS)		\
	$(top_builddir)/librsvg-2.la 	\
	$(LIBRSVG_LIBS) 		\
	$(POPT_LIBS)

rsvg_SOURCES=test-rsvg.c
rsvg_LDFLAGS =
rsvg_DEPENDENCIES = $(DEPS)
rsvg_LDADD = $(LDADDS) $(libm)

test_performance_SOURCES=test-performance.c
test_performance_LDFLAGS =
test_performance_DEPENDENCIES = $(DEPS)
test_performance_LDADD = $(LDADDS) $(libm)

rsvg_view_SOURCES = 		\
	test-display.c
rsvg_view_LDFLAGS =
rsvg_view_DEPENDENCIES = $(DEPS)
rsvg_view_LDADD = $(LDADDS) $(GTK_LIBS) $(GDK_X11_LIBS) $(libm)

EXTRA_DIST =				\
	COPYING.LIB 			\
	librsvg.spec.in 		\
	librsvg-2.0.pc.in 		\
	librsvg-2.0-uninstalled.pc.in 	\
	librsvg-zip.in 			\
	rsvg-gz.c 			\
	rsvg-gz.h 			\
	$(man_MANS)			\
	$(rsvg_view_SOURCES)

tests: rsvg

## Put `exec' in the name because this should be installed by
## `install-exec', not `install-data'.

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-2.0.pc

dist-hook: librsvg.spec
	cp librsvg.spec $(distdir)
