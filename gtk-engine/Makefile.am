DIST_SUBDIRS= examples

INCLUDES = $(GTK_CFLAGS) $(LIBRSVG_CFLAGS) -I$(top_srcdir)

enginedir = $(libdir)/gtk-2.0/$(GTK_VERSION)/engines
themedir = $(datadir)/themes

if ENABLE_GTK_ENGINE
target_rsvg_engine = libsvg.la
SUBDIRS =
else
target_rsvg_engine =
SUBDIRS =
endif

engine_LTLIBRARIES = $(target_rsvg_engine)

libsvg_la_SOURCES = 		\
	svg-draw.c		\
	svg-main.c		\
	svg-render.c		\
	svg-rc-style.c		\
	svg-rc-style.h		\
	svg-style.h		\
	svg.h

libsvg_la_LDFLAGS =  -avoid-version -module
libsvg_la_LIBADD = $(GTK_LIBS) $(LIBRSVG_LIBS)
