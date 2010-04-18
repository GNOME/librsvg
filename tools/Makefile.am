if BUILD_MISC_TOOLS
noinst_PROGRAMS = test-performance rsvg-dimensions
endif

INCLUDES =				\
	-I$(top_srcdir) 		\
	-I$(top_builddir) 		\
	$(LIBGSF_CFLAGS)		\
	$(LIBCROCO_CFLAGS)		\
	$(LIBRSVG_CFLAGS) 		\
	$(GTK_CFLAGS)			\
	$(GDK_X11_CFLAGS)		\
	-DG_LOG_DOMAIN=\"librsvg\" 	\
	-DLIBRSVG_DATADIR="\"$(datadir)\""

DEPS = $(top_builddir)/librsvg-2.la
LDADDS =				\
	$(LIBGSF_LIBS)			\
	$(GLIB_LIBS)			\
	$(LIBCROCO_LIBS)		\
	$(LIBRSVG_LIBS) 		\
	$(top_builddir)/librsvg-2.la

test_performance_SOURCES = test-performance.c
test_performance_LDFLAGS =
test_performance_DEPENDENCIES = $(DEPS)
test_performance_LDADD = $(LDADDS) $(libm)

rsvg_dimensions_SOURCES = rsvg-dimensions.c
rsvg_dimensions_LDFLAGS =
rsvg_dimensions_DEPENDENCIES = $(DEPS)
rsvg_dimensions_LDADD = $(LDADDS) $(libm)

