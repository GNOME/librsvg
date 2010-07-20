if ENABLE_PIXBUF_LOADER

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
	$(LIBCROCO_CFLAGS)	\
	$(LIBGSF_CFLAGS)	\
	$(AM_CFLASG)

libpixbufloader_svg_la_LDFLAGS = -avoid-version -module $(AM_LDFLAGS)

libpixbufloader_svg_la_LIBADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(GDK_PIXBUF_LIBS)	\
	$(LIBRSVG_LIBS) 	\
	$(LIBCROCO_LIBS)	\
	$(LIBGSF_LIBS) 

if PLATFORM_WIN32
libpixbufloader_svg_la_LDFLAGS += -no-undefined
endif

install-data-hook:
	if test -z "$(DESTDIR)"; then \
		$(mkinstalldirs) $(DESTDIR)$(gdk_pixbuf_binarydir) ; \
		$(GDK_PIXBUF_QUERYLOADERS) > $(DESTDIR)$(gdk_pixbuf_cache_file) ; \
	fi

endif # ENABLE_PIXBUF_LOADER
