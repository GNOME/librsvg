include $(top_srcdir)/glib-tap.mk

test_programs = api

api_SOURCES = 		\
	api.c		\
	test-utils.c	\
	test-utils.h	\
	$(NULL)

api_LDADD = $(top_builddir)/librsvg_c_api.la	\
	$(LIBRSVG_LIBS)				\
	$(LIBM)

api_LDFLAGS = -static

AM_CPPFLAGS =					\
	-I$(srcdir) 				\
	-I$(top_srcdir)/include			\
	-I$(top_builddir)/include		\
	-DTEST_DATA_DIR="\"$(srcdir)\""		\
	-DTEST_SRC_DIR="\"$(PWD)\""		\
	-DTOP_SRC_DIR="\"$(top_srcdir)\""	\
	$(LIBRSVG_CFLAGS)

clean-local:
	rm -rf output
