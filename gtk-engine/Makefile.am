SUBDIRS =
DIST_SUBDIRS = examples

if ENABLE_GTK_ENGINE

SUBDIRS += examples

gtk2_enginedir = $(libdir)/gtk-2.0/$(GTK2_BINARY_VERSION)/engines
gtk3_enginedir = $(libdir)/gtk-3.0/$(GTK3_BINARY_VERSION)/engines

themedir = $(datadir)/themes

if HAVE_GTK_2
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
	$(LIBRSVG_CFLAGS)	\
	$(LIBCROCO_CFLAGS) 	\
	$(LIBGSF_CFLAGS) 	\
	$(AM_CFLAGS)

libsvg_la_LDFLAGS =  -avoid-version -module $(AM_LDFLAGS)
libsvg_la_LIBADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK2_LIBS) 		\
	$(LIBRSVG_LIBS)		\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS)

if PLATFORM_WIN32
libsvg_la_LDFLAGS += -no-undefined
endif

endif # HAVE_GTK_2

if HAVE_GTK_3
gtk3_engine_LTLIBRARIES = libsvg-3.la

libsvg_3_la_SOURCES = 		\
	svg-draw.c		\
	svg-main.c		\
	svg-render.c		\
	svg-rc-style.c		\
	svg-rc-style.h		\
	svg-style.h		\
	svg.h

libsvg_3_la_CPPFLAGS = \
	-I$(top_srcdir)		\
	-I$(top_builddir)	\
	-DG_LOG_DOMAIN=\"rsvg-engine-3\" \
	$(AM_CPPFLAGS)

libsvg_3_la_CFLAGS = \
	$(GTK3_CFLAGS)		\
	$(LIBRSVG_CFLAGS)	\
	$(LIBCROCO_CFLAGS) 	\
	$(LIBGSF_CFLAGS) 	\
	$(AM_CFLAGS)

libsvg_3_la_LDFLAGS =  -avoid-version -module $(AM_LDFLAGS)

libsvg_3_la_LIBADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK3_LIBS) 		\
	$(LIBRSVG_LIBS) 	\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS)

if PLATFORM_WIN32
libsvg_3_la_LDFLAGS += -no-undefined
endif

endif # HAVE_GTK_3

endif # ENABLE_GTK_ENGINE
