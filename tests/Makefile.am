SUBDIRS=pdiff .

TESTS = 		\
	rsvg-test	\
	dimensions

LDADD = $(top_builddir)/librsvg-2.la		\
	$(top_builddir)/tests/pdiff/libpdiff.la

INCLUDES = -I$(srcdir) 				\
	   -I$(top_srcdir)			\
	   -I$(srcdir)/pdiff			\
	   -DTEST_DATA_DIR="\"$(srcdir)\""	\
	  $(LIBRSVG_CFLAGS)

EXTRA_PROGRAMS = $(TESTS)

CLEANFILES = $(TESTS)

DISTCLEANFILES = rsvg-test.html			\
		 rsvg-test.log

EXTRA_DIST =					\
	fixtures/dimensions/bug564527.svg	\
	fixtures/dimensions/bug614018.svg	\
	fixtures/dimensions/bug612951.svg	\
	fixtures/dimensions/bug608102.svg	\
	fixtures/dimensions/sub-rect.svg

test:
	@$(MAKE) $(AM_MAKEFLAGS) check;

.PHONY: test
