SUBDIRS=pdiff .

TESTS = rsvg-test

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

test:
	@$(MAKE) $(AM_MAKEFLAGS) check;

.PHONY: test
