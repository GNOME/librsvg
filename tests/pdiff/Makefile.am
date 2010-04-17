if BUILD_MISC_TOOLS

noinst_PROGRAMS = perceptualdiff

noinst_LTLIBRARIES = libpdiff.la
libpdiff_la_SOURCES = 		\
	pdiff.h			\
	lpyramid.c		\
	lpyramid.h		\
	pdiff.c

perceptualdiff_SOURCES =	\
	args.c			\
	args.h			\
	perceptualdiff.c

INCLUDES = $(LIBRSVG_CFLAGS)
LDADD = libpdiff.la $(LIBRSVG_LIBS) -lm

endif

