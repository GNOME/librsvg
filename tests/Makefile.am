include $(top_srcdir)/glib-tap.mk

test_programs =		\
	loading		\
	rsvg-test	\
	crash		\
	styles		\
	render-crash	\
	dimensions

test_utils_common_sources = \
	test-utils.c	\
	test-utils.h

rsvg_test_SOURCES = \
	rsvg-test.c	\
	$(test_utils_common_sources)

crash_SOURCES = \
	crash.c		\
	$(test_utils_common_sources)

render_crash_SOURCES = \
	render-crash.c	\
	$(test_utils_common_sources)

styles_SOURCES = \
	styles.c	\
	$(test_utils_common_sources)

dimensions_SOURCES = \
	dimensions.c	\
	$(test_utils_common_sources)

loading_SOURCES =		 	\
	loading.c			\
	$(test_utils_common_sources)

LDADD = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la		\
	$(LIBRSVG_LIBS)							\
	$(LIBM)

INCLUDES = -I$(srcdir) 							\
	   -I$(top_srcdir)						\
	   -DTEST_DATA_DIR="\"$(srcdir)\""				\
	   -DTEST_SRC_DIR="\"$(PWD)\""					\
	   -DTOP_SRC_DIR="\"$(top_srcdir)\""				\
	  $(LIBRSVG_CFLAGS)

dist_installed_test_data =						\
	$(wildcard $(srcdir)/resources/*)				\
	$(wildcard $(srcdir)/fixtures/crash/*.svg)			\
	$(wildcard $(srcdir)/fixtures/crash/*.png)			\
	$(wildcard $(srcdir)/fixtures/loading/*)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.svg)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.png)			\
	$(wildcard $(srcdir)/fixtures/reftests/bugs/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/bugs/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/resources/*)	\
	$(wildcard $(srcdir)/fixtures/render-crash/*.svg)		\
	fixtures/dimensions/bug564527.svg				\
	fixtures/dimensions/bug614018.svg				\
	fixtures/dimensions/bug612951.svg				\
	fixtures/dimensions/bug608102.svg				\
	fixtures/dimensions/sub-rect-no-unit.svg			\
	fixtures/styles/bug620693.svg					\
	fixtures/styles/bug614704.svg					\
	fixtures/styles/bug614606.svg					\
	fixtures/styles/important.svg					\
	fixtures/styles/svg-class.svg					\
	fixtures/styles/bug592207.svg					\
	fixtures/styles/bug338160.svg					\
	fixtures/styles/bug379629.svg					\
	fixtures/styles/bug614643.svg					\
	fixtures/styles/bug418823.svg					\
	fixtures/styles/order.svg
