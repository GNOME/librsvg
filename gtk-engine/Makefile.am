INCLUDES = $(GTK_CFLAGS) $(LIBRSVG_CFLAGS)

enginedir = $(libdir)/gtk-2.0/$(GTK_VERSION)/engines
themedir = $(datadir)/themes

if ENABLE_GTK_ENGINE
target_rsvg_engine = libsvg.la
else
target_rsvg_engine =
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

dist-hook:
	cp -pr examples $(themedir);
	find $(distdir)/examples -name 'CVS' -print | xargs rm -rf 
