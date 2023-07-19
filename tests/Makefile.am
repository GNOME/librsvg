include $(top_srcdir)/glib-tap.mk

test_sources =				\
	src/api.rs			\
	src/bugs.rs			\
	src/compare_surfaces.rs		\
	src/errors.rs			\
	src/filters.rs			\
	src/geometries.rs		\
	src/intrinsic_dimensions.rs	\
	src/legacy_sizing.rs		\
	src/loading_crash.rs		\
	src/loading_disallowed.rs	\
	src/main.rs			\
	src/primitive_geometries.rs	\
	src/primitives.rs		\
	src/reference.rs		\
	src/reference_utils.rs		\
	src/render_crash.rs		\
	src/shapes.rs			\
	src/text.rs			\
	src/utils.rs			\
	src/cmdline/mod.rs		\
	src/cmdline/rsvg_convert.rs	\
	src/predicates/file.rs		\
	src/predicates/mod.rs		\
	src/predicates/pdf.rs		\
	src/predicates/png.rs		\
	src/predicates/svg.rs		\
	$(NULL)

test_programs = api

api_SOURCES = 		\
	api.c		\
	test-utils.c	\
	test-utils.h	\
	$(NULL)

LDADD = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(LIBRSVG_LIBS)						\
	$(LIBM)

AM_CPPFLAGS =					\
	-I$(srcdir) 				\
	-I$(top_srcdir)/include			\
	-I$(top_builddir)/include		\
	-DTEST_DATA_DIR="\"$(srcdir)\""		\
	-DTEST_SRC_DIR="\"$(PWD)\""		\
	-DTOP_SRC_DIR="\"$(top_srcdir)\""	\
	$(LIBRSVG_CFLAGS)

test_resources =	\
	$(wildcard $(srcdir)/resources/*.ttf)

test_fixtures =								\
	$(wildcard $(srcdir)/fixtures/api/*.svg)			\
	$(wildcard $(srcdir)/fixtures/cmdline/*.svg)			\
	$(wildcard $(srcdir)/fixtures/cmdline/*.png)			\
	$(wildcard $(srcdir)/fixtures/crash/*.svg)			\
	$(wildcard $(srcdir)/fixtures/crash/*.png)			\
	$(wildcard $(srcdir)/fixtures/errors/*)				\
	$(wildcard $(srcdir)/fixtures/geometries/*)			\
	$(wildcard $(srcdir)/fixtures/loading/*)			\
	$(wildcard $(srcdir)/fixtures/loading/subdir/*)			\
	$(wildcard $(srcdir)/fixtures/primitive_geometries/*)		\
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
	$(wildcard $(srcdir)/fixtures/reftests/bugs-reftests/*.svg)	\
	$(wildcard $(srcdir)/fixtures/reftests/svg2-reftests/*.svg)	\
	$(wildcard $(srcdir)/fixtures/render-crash/*.svg)		\
	$(wildcard $(srcdir)/fixtures/text/*.svg)		        \
	$(wildcard $(srcdir)/fixtures/dimensions/*.svg)

EXTRA_DIST +=			\
	$(test_sources)		\
	$(test_resources)	\
	$(test_fixtures)	\
	README.md		\
	$(NULL)

clean-local:
	rm -rf output
