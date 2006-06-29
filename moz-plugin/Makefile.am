plugindir = $(libdir)/mozilla/plugins

if WITH_MOZILLA
plugin_LTLIBRARIES = libmozsvgdec.la
else
plugin_LTLIBRARIES = 
endif

INCLUDES=$(MOZILLA_CFLAGS) -DBINDIR=\"$(bindir)/\" $(GLIB_CFLAGS) $(GDK_X11_CFLAGS)

libmozsvgdec_la_SOURCES = moz-plugin.c
libmozsvgdec_la_LDFLAGS = -module -avoid-version 
libmozsvgdec_la_LIBADD = $(GLIB_LIBS)
EXTRA_DIST=moz-plugin.c
