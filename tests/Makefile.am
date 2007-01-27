SUBDIRS=pdiff .

TESTS = rsvg-test


LDADD = $(top_builddir)/librsvg-2.la		\
	$(top_builddir)/tests/pdiff/libpdiff.la

INCLUDES = -I$(srcdir) 				\
	   -I$(srcdir)/pdiff			\
	  $(LIBRSVG_CFLAGS)

EXTRA_PROGRAMS = $(TESTS)

test:
	@$(MAKE) $(AM_MAKEFLAGS) check;
