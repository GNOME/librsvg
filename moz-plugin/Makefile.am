plugindir = $(libdir)/mozilla/plugins/

if WITH_MOZILLA
plugin_LTLIBRARIES = libmozsvgdec.la
else
plugin_LTLIBRARIES = 
endif

INCLUDES=$(MOZILLA_CFLAGS) -DBINDIR=\"$(bindir)/\" $(GDK_X11_CFLAGS)

libmozsvgdec_la_SOURCES = moz-plugin.c
libmozsvgdec_la_LDFLAGS = -module -avoid-version 
libmozsvgdec_la_LIBADD = $(LIBRSVG_LIBS) \
	$(LIBGSF_LIBS) $(LIBCROCO_LIBS)\
	$(top_builddir)/librsvg-2.la

EXTRA_DIST=moz-plugin.c
