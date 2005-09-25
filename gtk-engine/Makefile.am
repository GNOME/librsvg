DIST_SUBDIRS= examples

if PLATFORM_WIN32
no_undefined = -no-undefined
endif

INCLUDES = $(GTK_CFLAGS) $(LIBGSF_CFLAGS) $(LIBCROCO_CFLAGS) $(LIBRSVG_CFLAGS) -I$(top_srcdir)

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

libsvg_la_LDFLAGS =  -avoid-version -module $(no_undefined)
libsvg_la_LIBADD = $(top_builddir)/librsvg-2.la $(GTK_LIBS) $(LIBRSVG_LIBS) $(LIBGNOME_VFS_LIBS) $(LIBCROCO_LIBS)

EXTRA_DIST = $(libsvg_la_SOURCES)
