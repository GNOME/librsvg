SUBDIRS = . gtk-engine gdk-pixbuf-loader doc gimp-plugin

bin_PROGRAMS = rsvg

noinst_PROGRAMS = test-performance #test-display

man_MANS = rsvg.1

if WITH_LIBGSF
GSFHEADERS = rsvg-gz.h
GSFSOURCES = rsvg-gz.c
else
GSFHEADERS = 
GSFSOURCES = 
endif

EXTRA_DIST =\
	COPYING.LIB \
	librsvg.spec.in \
	librsvg-2.0.pc.in \
	rsvg-gz.c \
	rsvg-gz.h \
	$(man_MANS)

lib_LTLIBRARIES = librsvg-2.la

librsvg_2_la_SOURCES = $(GSFSOURCES) \
	librsvg-features.c \
	rsvg-bpath-util.c \
	rsvg-bpath-util.h \
	rsvg-css.c \
	rsvg-css.h \
	rsvg-defs.c \
	rsvg-defs.h \
	rsvg-paint-server.c \
	rsvg-paint-server.h \
	rsvg-path.c \
	rsvg-path.h \
	rsvg-private.h \
	rsvg-file-util.c \
	rsvg-shapes.c	\
	rsvg-shapes.h	\
	rsvg-styles.c	\
	rsvg-styles.h	\
	rsvg-text.c	\
	rsvg-text.h	\
	rsvg.c

librsvg_2_la_LDFLAGS = -version-info @VERSION_INFO@ -no-undefined -export-dynamic
librsvg_2_la_LIBADD = $(LIBRSVG_LIBS)

librsvgincdir = $(includedir)/librsvg-2/librsvg
librsvginc_HEADERS = $(GSFHEADERS) \
	rsvg.h \
	librsvg-features.h

INCLUDES =$(LIBGSF_CFLAGS)\
	$(LIBCROCO_CFLAGS)\
	-I$(top_srcdir) \
	-I$(top_builddir) \
	$(LIBRSVG_CFLAGS) \
	$(GTK_CFLAGS)	\
	-DG_LOG_DOMAIN=\"librsvg\" \
	-DG_DISABLE_DEPRECATED \
	-DPANGO_DISABLE_DEPRECATED \
	-DGDK_DISABLE_DEPRECATED \
	-DGDK_PIXBUF_DISABLE_DEPRECATED \
	-DGTK_DISABLE_DEPRECATED \
	-DDATADIR="\"$(datadir)\""

DEPS = $(top_builddir)/librsvg-2.la
LDADDS =$(LIBGSF_LIBS)\
	$(LIBCROCO_LIBS)\
	$(top_builddir)/librsvg-2.la \
	$(LIBRSVG_LIBS) \
	$(POPT_LIBS)

rsvg_SOURCES=test-rsvg.c
rsvg_LDFLAGS =
rsvg_DEPENDENCIES = $(DEPS)
rsvg_LDADD = $(LDADDS) -lm

test_performance_SOURCES=test-performance.c
test_performance_LDFLAGS =
test_performance_DEPENDENCIES = $(DEPS)
test_performance_LDADD = $(LDADDS) -lm

#test_display_SOURCES=test-display.c
#test_display_LDFLAGS =
#test_display_DEPENDENCIES = $(DEPS)
#test_display_LDADD = $(LDADDS) $(GTK_LIBS) -lm

tests: rsvg

## Put `exec' in the name because this should be installed by
## `install-exec', not `install-data'.

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-2.0.pc

dist-hook: librsvg.spec
	cp librsvg.spec $(distdir)
