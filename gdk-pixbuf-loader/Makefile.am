
loaderdir = $(libdir)/gtk-2.0/$(GTK_VERSION)/loaders

if PLATFORM_WIN32
no_undefined = -no-undefined
endif

INCLUDES = \
	$(LIBRSVG_CFLAGS) \
	$(LIBGSF_CFLAGS) \
	$(LIBCROCO_CFLAGS)\
	-DGDK_PIXBUF_ENABLE_BACKEND \
	-I$(top_srcdir)

if ENABLE_PIXBUF_LOADER
target_pixbuf_loader = svg_loader.la 
else
target_pixbuf_loader = 
endif

svg_loader_la_SOURCES = io-svg.c
svg_loader_la_LDFLAGS = -avoid-version -module $(no_undefined)
svg_loader_la_LIBADD = \
	$(top_builddir)/librsvg-2.la	\
	$(LIBRSVG_LIBS) 	\
	$(LIBGNOME_VFS_LIBS)		\
	$(LIBGSF_LIBS) $(LIBCROCO_LIBS)

loader_LTLIBRARIES = $(target_pixbuf_loader)

install-data-hook: 
	if [ -z "$(DESTDIR)" ] ; then \
	  $(mkinstalldirs) $(DESTDIR)$(sysconfdir)/gtk-2.0 ; \
	  $(QUERYLOADERS) > $(DESTDIR)$(sysconfdir)/gtk-2.0/gdk-pixbuf.loaders ; \
	fi

