if ENABLE_PIXBUF_LOADER

NULL =

gdk_pixbuf_module_LTLIBRARIES = libpixbufloader-svg.la

libpixbufloader_svg_la_SOURCES = \
	io-svg.c

libpixbufloader_svg_la_CPPFLAGS = \
	-I$(top_srcdir)				\
	-I$(top_builddir)			\
	-I$(top_builddir)/librsvg		\
	-DGDK_PIXBUF_ENABLE_BACKEND		\
	-DG_LOG_DOMAIN=\"libpixbufloader-svg\" 	\
	$(AM_CPPFLAGS)

libpixbufloader_svg_la_CFLAGS = \
	$(GDK_PIXBUF_CFLAGS)	\
	$(LIBRSVG_CFLAGS)	\
	$(AM_CFLAGS)

libpixbufloader_svg_la_LDFLAGS = -avoid-version -module $(AM_LDFLAGS)

libpixbufloader_svg_la_LIBADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(GDK_PIXBUF_LIBS)	\
	$(LIBRSVG_LIBS)

if PLATFORM_WIN32
libpixbufloader_svg_la_LDFLAGS += -no-undefined
endif

noinst_PROGRAMS = rsvg-loader

rsvg_loader_SOURCES = \
	test.c \
	$(NULL)
rsvg_loader_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir)			\
	$(AM_CPPFLAGS)
rsvg_loader_CFLAGS = \
	$(GDK_PIXBUF_CFLAGS) \
	$(AM_CFLAGS)
rsvg_loader_LDFLAGS = \
	$(AM_LDFLAGS)
rsvg_loader_LDADD = \
	$(GDK_PIXBUF_LIBS)
EXTRA_rsvg_loader_DEPENDENCIES = libpixbufloader-svg.la gdk-pixbuf-loaders

gdk-pixbuf-loaders: Makefile
	$(AM_V_GEN) ( $(GDK_PIXBUF_QUERYLOADERS) ./libpixbufloader-svg.la && $(GDK_PIXBUF_QUERYLOADERS)) > gdk-pixbuf.loaders 2>/dev/null

if CROSS_COMPILING
RUN_QUERY_LOADER_TEST=false
else
RUN_QUERY_LOADER_TEST=test -z "$(DESTDIR)"
endif

# The following bit is stolen from gdk-pixbuf/gdk-pixbuf/Makefile.am:
#
# Running this if cross compiling or if DESTDIR is set is going to
# not work at all, so skip it
# We use install-data-hook here to workaround a bug in automake and/or libtool
# that makes the install target for the loader libraries a dependency on
# install-data-am, and not install-exec-am. We need to ensure this gets run
# after the libraries are installed in their final locations.
install-data-hook:
	@if $(RUN_QUERY_LOADER_TEST) ; then \
		$(mkinstalldirs) $(DESTDIR)$(gdk_pixbuf_binarydir) ; \
		$(GDK_PIXBUF_QUERYLOADERS) > $(DESTDIR)$(gdk_pixbuf_cache_file) ; \
	else \
	  echo "***" ; \
	  echo "*** Warning: loaders.cache not built" ; \
	  echo "***" ; \
	  echo "*** Generate this file manually on host" ; \
	  echo "*** system using gdk-pixbuf-query-loaders" ; \
	  echo "***" ; \
	fi

thumbnailerdir = $(datadir)/thumbnailers/
thumbnailer_DATA = librsvg.thumbnailer
librsvg.thumbnailer: librsvg.thumbnailer.in
	$(AM_V_GEN) $(SED) -e "s|\@bindir\@|$(bindir)|" \
	$< > $@

EXTRA_DIST = librsvg.thumbnailer.in

CLEANFILES = \
	gdk-pixbuf.loaders \
	$(thumbnailer_DATA) \
	$(NULL)

DISTCLEANFILES = \
	gdk-pixbuf.loaders \
	$(NULL)

endif # ENABLE_PIXBUF_LOADER
