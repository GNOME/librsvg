gimpdir = $(libdir)/gimp/1.3/plug-ins

INCLUDES = \
	$(LIBRSVG_CFLAGS) \
	$(LIBGIMP_CFLAGS) \
	-I$(top_srcdir)

if ENABLE_GIMP_PLUGIN
gimp_PROGRAMS = svg
else
gimp_PROGRAMS =
endif

DEPS = $(top_builddir)/librsvg-2.la
LDADDS =$(LIBGSF_LIBS)\
	$(LIBGIMP_LIBS)\
	$(top_builddir)/librsvg-2.la \
	$(LIBRSVG_LIBS)

svg_SOURCES=svg.c
svg_LDFLAGS = -avoid-version -module
svg_DEPENDENCIES = $(DEPS)
svg_LDADD = $(LDADDS) -lm

install-%: %
	$(mkinstalldirs) $(gimpdir)
