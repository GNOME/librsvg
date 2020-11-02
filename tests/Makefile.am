include $(top_srcdir)/glib-tap.mk

NULL =

test_sources =					\
	build.rs				\
	src/api.rs				\
	src/bugs.rs				\
	src/cmdline/mod.rs			\
	src/cmdline/rsvg_convert.rs		\
	src/errors.rs				\
	src/intrinsic_dimensions.rs		\
	src/loading_crash.rs			\
	src/main.rs				\
	src/predicates/file.rs			\
	src/predicates/mod.rs			\
	src/predicates/pdf.rs			\
	src/predicates/png.rs			\
	src/primitives.rs			\
	src/reference.rs			\
	src/render_crash.rs			\
	src/utils.rs				\
	$(NULL)

EXTRA_DIST +=		\
	$(test_sources)	\
	README.md	\
	$(NULL)

test_programs = api

api_SOURCES = 		\
	api.c		\
	test-utils.c	\
	test-utils.h	\
	$(NULL)

LDADD = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la		\
	$(LIBRSVG_LIBS)							\
	$(LIBM)

AM_CPPFLAGS =					\
	-I$(srcdir) 				\
	-I$(top_srcdir)				\
	-I$(top_builddir)/include		\
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
	$(wildcard $(srcdir)/fixtures/loading/*)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.css)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.svg)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.png)			\
	$(wildcard $(srcdir)/fixtures/reftests/*.txt)			\
	$(wildcard $(srcdir)/fixtures/reftests/adwaita/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/adwaita/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/bugs/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/bugs/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/*.png)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/images/*)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg1.1/resources/*)	\
	$(wildcard $(srcdir)/fixtures/reftests/svg2/*.svg)		\
	$(wildcard $(srcdir)/fixtures/reftests/svg2/*.png)		\
	$(wildcard $(srcdir)/fixtures/render-crash/*.svg)		\
	$(wildcard $(srcdir)/fixtures/dimensions/*.svg)

clean-local:
	rm -rf output
