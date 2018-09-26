include $(top_srcdir)/glib-tap.mk

EXTRA_DIST +=		\
	README.md

# Keep "errors" at the end; they are the slowest to run
test_programs =		\
	api		\
	loading		\
	rsvg-test	\
	crash		\
	render-crash	\
	dimensions	\
	errors		\
	infinite-loop

# Removed "styles" from the above; it is broken right now

test_utils_common_sources = \
	test-utils.c	\
	test-utils.h

api_SOURCES = 		\
	api.c		\
	$(test_utils_common_sources)

errors_SOURCES = 	\
	errors.c	\
	$(test_utils_common_sources)

infinite_loop_SOURCES = 	\
	infinite-loop.c		\
	$(test_utils_common_sources)

rsvg_test_SOURCES = \
	rsvg-test.c	\
	$(test_utils_common_sources)

crash_SOURCES = \
	crash.c		\
	$(test_utils_common_sources)

render_crash_SOURCES = \
	render-crash.c	\
	$(test_utils_common_sources)

# styles_SOURCES = \
# 	styles.c	\
# 	$(test_utils_common_sources)

dimensions_SOURCES = \
	dimensions.c	\
	$(test_utils_common_sources)

loading_SOURCES =		 	\
	loading.c			\
	$(test_utils_common_sources)

LDADD = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la		\
	$(LIBRSVG_LIBS)							\
	$(LIBM)

AM_CPPFLAGS =					\
	-I$(srcdir) 				\
	-I$(top_srcdir)				\
	-I$(top_builddir)/librsvg		\
	-DTEST_DATA_DIR="\"$(srcdir)\""		\
	-DTEST_SRC_DIR="\"$(PWD)\""		\
	-DTOP_SRC_DIR="\"$(top_srcdir)\""	\
	$(LIBRSVG_CFLAGS)

dist_installed_test_data =						\
	$(wildcard $(srcdir)/resources/*)				\
	$(wildcard $(srcdir)/fixtures/api/*.svg)			\
	$(wildcard $(srcdir)/fixtures/crash/*.svg)			\
	$(wildcard $(srcdir)/fixtures/crash/*.png)			\
	$(wildcard $(srcdir)/fixtures/errors/*)				\
	$(wildcard $(srcdir)/fixtures/infinite-loop/*)			\
	$(wildcard $(srcdir)/fixtures/loading/*)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.css)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.svg)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.png)			\
	$(wildcard $(srcdir)/fixtures/reftests/bugs/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/bugs/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/images/*)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/resources/*)	\
	$(wildcard $(srcdir)/fixtures/render-crash/*.svg)		\
	$(wildcard $(srcdir)/fixtures/dimensions/*.svg)			\
	$(wildcard $(srcdir)/fixtures/styles/*.svg)

clean-local:
	rm -rf output
