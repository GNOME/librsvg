gimpdir = $(libdir)/gimp/1.3/plug-ins

if PLATFORM_WIN32
no_undefined = -no-undefined
endif

INCLUDES = $(GTK_CFLAGS)\
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
	$(LIBCROCO_LIBS)\
	$(LIBGIMP_LIBS)\
	$(GTK_LIBS)\
	$(top_builddir)/librsvg-2.la \
	$(LIBRSVG_LIBS)

svg_SOURCES=svg.c
svg_LDFLAGS = -avoid-version -module $(no_undefined)
svg_DEPENDENCIES = $(DEPS)
svg_LDADD = $(LDADDS) -lm

install-%: %
	$(mkinstalldirs) $(gimpdir)
