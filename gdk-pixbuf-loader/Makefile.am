loaderdir = $(libdir)/gtk-$(GTK_API_VERSION)/$(GTK_BINARY_VERSION)/loaders

if PLATFORM_WIN32
no_undefined = -no-undefined
endif

INCLUDES = \
	-I$(top_srcdir) \
	$(LIBRSVG_CFLAGS) \
	$(LIBGSF_CFLAGS) \
	$(LIBCROCO_CFLAGS)\
	-DGDK_PIXBUF_ENABLE_BACKEND

if ENABLE_PIXBUF_LOADER
target_pixbuf_loader = svg_loader.la 
else
target_pixbuf_loader = 
endif

svg_loader_la_SOURCES = io-svg.c
svg_loader_la_LDFLAGS = -avoid-version -module $(no_undefined)
svg_loader_la_LIBADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(LIBRSVG_LIBS) 	\
	$(LIBGSF_LIBS) $(LIBCROCO_LIBS)

loader_LTLIBRARIES = $(target_pixbuf_loader)

install-data-hook: 
	if [ -z "$(DESTDIR)" ] ; then \
	  $(mkinstalldirs) $(DESTDIR)$(sysconfdir)/gtk-$(GTK_API_VERSION) ; \
	  $(GDK_PIXBUF_QUERYLOADERS) > $(DESTDIR)$(sysconfdir)/gtk-$(GTK_API_VERSION)/gdk-pixbuf.loaders ; \
	fi

