if BUILD_MISC_TOOLS
noinst_PROGRAMS = 			\
	rsvg-dimensions			\
	test-performance		\
	test-memory

noinst_LTLIBRARIES = 			\
	librsvg_tools_main.la
endif

INCLUDES =				\
	-I$(top_srcdir) 		\
	-I$(top_builddir) 		\
	$(LIBRSVG_CFLAGS) 		\
	$(GTK_CFLAGS)			\
	$(GDK_X11_CFLAGS)		\
	-DG_LOG_DOMAIN=\"librsvg\" 	\
	-DLIBRSVG_DATADIR="\"$(datadir)\""

DEPS = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	librsvg_tools_main.la
LDADDS =				\
	$(GLIB_LIBS)			\
	$(LIBRSVG_LIBS) 		\
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la

librsvg_tools_main_la_SOURCES = 		\
	rsvg-tools-main.c		\
	rsvg-tools-main.h
librsvg_tools_main_la_LIBADD =

test_performance_SOURCES = test-performance.c
test_performance_LDFLAGS =
test_performance_DEPENDENCIES = $(DEPS)
test_performance_LDADD = librsvg_tools_main.la $(LDADDS) $(LIBM)

rsvg_dimensions_SOURCES = rsvg-dimensions.c
rsvg_dimensions_LDFLAGS =
rsvg_dimensions_DEPENDENCIES = $(DEPS)
rsvg_dimensions_LDADD = $(LDADDS) $(LIBM)

test_memory_SOURCES = test-memory.c
test_memory_LDFLAGS =
test_memory_DEPENDENCIES = $(DEPS)
test_memory_LDADD = librsvg_tools_main.la $(LDADDS) $(LIBM)
