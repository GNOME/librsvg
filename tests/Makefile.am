SUBDIRS=pdiff .

TESTS = 		\
	rsvg-test	\
	crash		\
	dimensions	\
	styles

noinst_LTLIBRARIES = 			\
	libtest-utils.la

LDADD = $(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la		\
	$(top_builddir)/tests/libtest-utils.la	\
	$(top_builddir)/tests/pdiff/libpdiff.la

INCLUDES = -I$(srcdir) 							\
	   -I$(top_srcdir)						\
	   -I$(srcdir)/pdiff						\
	   -DTEST_DATA_DIR="\"$(srcdir)\""				\
	   -DTEST_SRC_DIR="\"$(PWD)\""					\
	   -DTOP_SRC_DIR="\"$(top_srcdir)\""				\
	  $(LIBRSVG_CFLAGS)

EXTRA_PROGRAMS = $(TESTS)

CLEANFILES = $(TESTS)

DISTCLEANFILES = rsvg-test.html			\
		 rsvg-test.log

libtest_utils_la_SOURCES = 				\
	test-utils.c	   				\
	test-utils.h					\
	$(top_srcdir)/rsvg-size-callback.c		\
	$(top_srcdir)/rsvg-size-callback.h

EXTRA_DIST =						\
	fixtures/crash/bug620238.svg			\
	fixtures/dimensions/bug564527.svg		\
	fixtures/dimensions/bug614018.svg		\
	fixtures/dimensions/bug612951.svg		\
	fixtures/dimensions/bug608102.svg		\
	fixtures/dimensions/sub-rect-no-unit.svg	\
	fixtures/styles/bug620693.svg			\
	fixtures/styles/bug614704.svg			\
	fixtures/styles/bug614606.svg			\
	fixtures/styles/important.svg			\
	fixtures/styles/svg-class.svg			\
	fixtures/styles/bug592207.svg			\
	fixtures/styles/bug338160.svg			\
	fixtures/styles/bug379629.svg			\
	fixtures/styles/bug614643.svg			\
	fixtures/styles/bug418823.svg			\
	fixtures/styles/order.svg

test:
	@$(MAKE) $(AM_MAKEFLAGS) check;

.PHONY: test
