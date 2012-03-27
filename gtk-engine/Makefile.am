SUBDIRS =
DIST_SUBDIRS = examples

SUBDIRS += examples

gtk2_enginedir = $(libdir)/gtk-2.0/$(GTK2_BINARY_VERSION)/engines

themedir = $(datadir)/themes

gtk2_engine_LTLIBRARIES = libsvg.la

libsvg_la_SOURCES = 		\
	svg-draw.c		\
	svg-main.c		\
	svg-render.c		\
	svg-rc-style.c		\
	svg-rc-style.h		\
	svg-style.h		\
	svg.h

libsvg_la_CPPFLAGS = \
	-I$(top_srcdir)		\
	-I$(top_builddir)	\
	-DG_LOG_DOMAIN=\"rsvg-engine-2\" \
	$(AM_CPPFLAGS)

libsvg_la_CFLAGS = \
	$(GTK2_CFLAGS)		\
	$(GMODULE_CFLAGS)	\
	$(LIBRSVG_CFLAGS)	\
	$(LIBCROCO_CFLAGS) 	\
	$(AM_CFLAGS)

libsvg_la_LDFLAGS =  -avoid-version -module $(AM_LDFLAGS)
libsvg_la_LIBADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK2_LIBS) 		\
	$(GMODULE_LIBS)		\
	$(LIBRSVG_LIBS)		\
	$(LIBCROCO_LIBS)

if PLATFORM_WIN32
libsvg_la_LDFLAGS += -no-undefined
endif
