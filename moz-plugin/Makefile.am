
plugindir = $(libdir)/mozilla/plugins/

if WITH_MOZILLA
plugin_LTLIBRARIES = libmozsvgdec.la
else
plugin_LTLIBRARIES = 
endif

libmozsvgdec_la_SOURCES = moz-plugin.c
libmozsvgdec_la_CFLAGS = $(MOZILLA_CFLAGS) $(X_CFLAGS) -DBINDIR=\"$(bindir)\"
libmozsvgdec_la_LDFLAGS = -module -avoid-version $(X_LIBS)
libmozsvgdec_la_LIBADD = -lXt $(LIBRSVG_LIBS) \
	$(LIBGSF_LIBS) $(LIBCROCO_LIBS)\
	$(top_builddir)/librsvg-2.la

