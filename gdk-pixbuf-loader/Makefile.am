if ENABLE_PIXBUF_LOADER

NULL =

gdk_pixbuf_module_LTLIBRARIES = libpixbufloader-svg.la

libpixbufloader_svg_la_SOURCES = \
	io-svg.c

libpixbufloader_svg_la_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir)			\
	-DGDK_PIXBUF_ENABLE_BACKEND		\
	-DG_LOG_DOMAIN=\"libpixbufloader-svg\" 	\
	$(AM_CPPFLAGS)

libpixbufloader_svg_la_CFLAGS = \
	$(GDK_PIXBUF_CFLAGS)	\
	$(LIBRSVG_CFLAGS)	\
	$(AM_CFLASG)

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
	$(AM_CPPFLAGS)
rsvg_loader_CFLAGS = \
	$(GDK_PIXBUF_CFLAGS) \
	$(AM_CFLAGS)
rsvg_loader_LDFLAGS = \
	$(AM_LDFLAGS)
rsvg_loader_LDADD = \
	$(GDK_PIXBUF_LIBS)
rsvg_loader_DEPENDENCIES = libpixbufloader-svg.la gdk-pixbuf-loaders

gdk-pixbuf-loaders: Makefile
	$(AM_V_GEN) ( $(GDK_PIXBUF_QUERYLOADERS) ./libpixbufloader-svg.la && $(GDK_PIXBUF_QUERYLOADERS)) > gdk-pixbuf.loaders 2>/dev/null

install-data-hook:
	if test -z "$(DESTDIR)"; then \
		$(mkinstalldirs) $(DESTDIR)$(gdk_pixbuf_binarydir) ; \
		$(GDK_PIXBUF_QUERYLOADERS) > $(DESTDIR)$(gdk_pixbuf_cache_file) ; \
	fi

CLEANFILES = \
	gdk-pixbuf.loaders \
	$(NULL)

DISTCLEANFILES = \
	gdk-pixbuf.loaders \
	$(NULL)

endif # ENABLE_PIXBUF_LOADER
