
loaderdir = $(libdir)/gtk-2.0/$(GTK_VERSION)/loaders

INCLUDES = @LIBRSVG_CFLAGS@ @GDK_PIXBUF_CFLAGS@ -DGDK_PIXBUF_ENABLE_BACKEND

if ENABLE_PIXBUF_LOADER
target_pixbuf_loader = svg_loader.la 
else
target_pixbuf_loader = 
endif

svg_loader_la_SOURCES = io-svg.c
svg_loader_la_LDFLAGS = -avoid-version -module
svg_loader_la_LIBADD = $(GDK_PIXBUF_LIBS) $(LIBRSVG_LIBS) $(top_builddir)/librsvg-2.la

loader_LTLIBRARIES = $(target_pixbuf_loader)

install-data-local: 
	if [ -z "$(DESTDIR)" ] ; then \
	  $(mkinstalldirs) $(DESTDIR)$(sysconfdir)/gtk-2.0 ; \
	  $(QUERYLOADERS) > $(DESTDIR)$(sysconfdir)/gtk-2.0/gdk-pixbuf.loaders ; \
	fi









